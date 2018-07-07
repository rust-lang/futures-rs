use futures_core::future::{Future, FutureObj, LocalFutureObj};
use futures_core::stream::{Stream};
use futures_core::task::{
    self, Context, Poll, LocalWaker, Wake,
    Executor, SpawnObjError, SpawnLocalObjError, SpawnErrorKind
};
use futures_util::stream::FuturesUnordered;
use futures_util::stream::StreamExt;
use std::cell::{RefCell};
use std::marker::Unpin;
use std::ops::{Deref, DerefMut};
use std::prelude::v1::*;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::thread::{self, Thread};

use crate::enter;
use crate::ThreadPool;

/// A single-threaded task pool.
///
/// This executor allows you to multiplex any number of tasks onto a single
/// thread. It's appropriate for strictly I/O-bound tasks that do very little
/// work in between I/O actions.
///
/// To get a handle to the pool that implements
/// [`Executor`](futures_core::task::Executor), use the
/// [`executor()`](LocalPool::executor) method. Because the executor is
/// single-threaded, it supports a special form of task spawning for non-`Send`
/// futures, via [`spawn_local_obj`](LocalExecutor::spawn_local_obj).
#[derive(Debug)]
pub struct LocalPool {
    pool: FuturesUnordered<LocalFutureObj<'static, ()>>,
    incoming: Rc<Incoming>,
}

/// A handle to a [`LocalPool`](LocalPool) that implements
/// [`Executor`](futures_core::task::Executor).
#[derive(Clone, Debug)]
pub struct LocalExecutor {
    incoming: Weak<Incoming>,
}

type Incoming = RefCell<Vec<LocalFutureObj<'static, ()>>>;

pub(crate) struct ThreadNotify {
    thread: Thread
}

thread_local! {
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        thread: thread::current(),
    });
}

impl Wake for ThreadNotify {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.thread.unpark();
    }
}

// Set up and run a basic single-threaded executor loop, invocing `f` on each
// turn.
fn run_executor<T, F: FnMut(&LocalWaker) -> Poll<T>>(mut f: F) -> T {
    let _enter = enter()
        .expect("cannot execute `LocalPool` executor from within \
                 another executor");

    CURRENT_THREAD_NOTIFY.with(|thread_notify| {
        let local_waker =
          task::local_waker_from_nonlocal(thread_notify.clone());
        loop {
            if let Poll::Ready(t) = f(&local_waker) {
                return t;
            }
            thread::park();
        }
    })
}

impl LocalPool {
    /// Create a new, empty pool of tasks.
    pub fn new() -> LocalPool {
        LocalPool {
            pool: FuturesUnordered::new(),
            incoming: Default::default(),
        }
    }

    /// Get a clonable handle to the pool as an executor.
    pub fn executor(&self) -> LocalExecutor {
        LocalExecutor {
            incoming: Rc::downgrade(&self.incoming)
        }
    }

    /// Run all tasks in the pool to completion.
    ///
    /// The given executor, `exec`, is used as the default executor for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its executor handle:
    ///
    /// ```
    /// # extern crate futures;
    /// # use futures::executor::LocalPool;
    ///
    /// # fn main() {
    /// let mut pool = LocalPool::new();
    /// let mut exec = pool.executor();
    ///
    /// // ... spawn some initial tasks using `exec.spawn()` or `exec.spawn_local()`
    ///
    /// // run *all* tasks in the pool to completion, including any newly-spawned ones.
    /// pool.run(&mut exec);
    /// # }
    /// ```
    ///
    /// The function will block the calling thread until *all* tasks in the pool
    /// are complete, including any spawned while running existing tasks.
    pub fn run<Exec>(&mut self, exec: &mut Exec) where Exec: Executor + Sized {
        run_executor(|local_waker| self.poll_pool(local_waker, exec))
    }

    /// Runs all the tasks in the pool until the given future completes.
    ///
    /// The given executor, `exec`, is used as the default executor for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its executor handle:
    ///
    /// ```
    /// # #![feature(pin, arbitrary_self_types, futures_api)]
    /// # extern crate futures;
    /// # use futures::executor::LocalPool;
    /// # use futures::future::ready;
    ///
    /// # fn main() {
    /// let mut pool = LocalPool::new();
    /// let mut exec = pool.executor();
    /// # let my_app  = ready(());
    ///
    /// // run tasks in the pool until `my_app` completes, by default spawning
    /// // further tasks back onto the pool
    /// pool.run_until(my_app, &mut exec);
    /// # }
    /// ```
    ///
    /// The function will block the calling thread *only* until the future `f`
    /// completes; there may still be incomplete tasks in the pool, which will
    /// be inert after the call completes, but can continue with further use of
    /// `run` or `run_until`. While the function is running, however, all tasks
    /// in the pool will try to make progress.
    pub fn run_until<F, Exec>(&mut self, future: F, exec: &mut Exec)
        -> F::Output
        where F: Future, Exec: Executor + Sized
    {
        pin_mut!(future);

        run_executor(|local_waker| {
            {
                let mut main_cx = Context::new(local_waker, exec);

                // if our main task is done, so are we
                match future.reborrow().poll(&mut main_cx) {
                    Poll::Ready(output) => return Poll::Ready(output),
                    _ => {}
                }
            }

            self.poll_pool(local_waker, exec);
            Poll::Pending
        })
    }

    // Make maximal progress on the entire pool of spawned task, returning `Ready`
    // if the pool is empty and `Pending` if no further progress can be made.
    fn poll_pool<Exec>(&mut self, local_waker: &LocalWaker, exec: &mut Exec)
        -> Poll<()>
        where Exec: Executor + Sized
    {
        // state for the FuturesUnordered, which will never be used
        let mut pool_cx = Context::new(local_waker, exec);

        loop {
            // empty the incoming queue of newly-spawned tasks
            {
                let mut incoming = self.incoming.borrow_mut();
                for task in incoming.drain(..) {
                    self.pool.push(task)
                }
            }

            let ret = self.pool.poll_next_unpin(&mut pool_cx);
            // we queued up some new tasks; add them and poll again
            if !self.incoming.borrow().is_empty() {
                continue;
            }

            // no queued tasks; we may be done
            match ret {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(()),
                _ => {}
            }
        }
    }
}

lazy_static! {
    static ref GLOBAL_POOL: ThreadPool = ThreadPool::builder()
        .name_prefix("block_on-")
        .create()
        .expect("Unable to create global thread-pool");
}

/// Run a future to completion on the current thread.
///
/// This function will block the caller until the given future has completed.
/// The default executor for the future is a global `ThreadPool`.
///
/// Use a [`LocalPool`](LocalPool) if you need finer-grained control over
/// spawned tasks.
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut pool = LocalPool::new();
    pool.run_until(f, &mut GLOBAL_POOL.clone())
}

/// Turn a stream into a blocking iterator.
///
/// When `next` is called on the resulting `BlockingStream`, the caller
/// will be blocked until the next element of the `Stream` becomes available.
/// The default executor for the future is a global `ThreadPool`.
pub fn block_on_stream<S: Stream + Unpin>(stream: S) -> BlockingStream<S> {
    BlockingStream { stream }
}

/// An iterator which blocks on values from a stream until they become available.
#[derive(Debug)]
pub struct BlockingStream<S: Stream + Unpin> { stream: S }

impl<S: Stream + Unpin> Deref for BlockingStream<S> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S: Stream + Unpin> DerefMut for BlockingStream<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl<S: Stream + Unpin> BlockingStream<S> {
    /// Convert this `BlockingStream` into the inner `Stream` type.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: Stream + Unpin> Iterator for BlockingStream<S> {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        LocalPool::new().run_until(self.stream.next(), &mut GLOBAL_POOL.clone())
    }
}

impl Executor for LocalExecutor {
    fn spawn_obj(
        &mut self,
        task: FutureObj<'static, ()>,
    ) -> Result<(), SpawnObjError> {
        if let Some(incoming) = self.incoming.upgrade() {
            incoming.borrow_mut().push(task.into());
            Ok(())
        } else {
            Err(SpawnObjError{ task, kind: SpawnErrorKind::shutdown() })
        }
    }

    fn status(&self) -> Result<(), SpawnErrorKind> {
        if self.incoming.upgrade().is_some() {
            Ok(())
        } else {
            Err(SpawnErrorKind::shutdown())
        }
    }
}

impl LocalExecutor {
    /// Spawn a non-`Send` future onto the associated [`LocalPool`](LocalPool).
    pub fn spawn_local_obj(
        &mut self,
        task: LocalFutureObj<'static, ()>,
    ) -> Result<(), SpawnLocalObjError> {
        if let Some(incoming) = self.incoming.upgrade() {
            incoming.borrow_mut().push(task);
            Ok(())
        } else {
            Err(SpawnLocalObjError{ task, kind: SpawnErrorKind::shutdown() })
        }
    }
}
