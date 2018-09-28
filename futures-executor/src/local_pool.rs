use crate::{enter, ThreadPool};
use futures_core::future::{Future, FutureObj, LocalFutureObj};
use futures_core::stream::{Stream};
use futures_core::task::{
    self, Poll, LocalWaker, Wake,
    Spawn, LocalSpawn, SpawnError,
};
use futures_util::stream::FuturesUnordered;
use futures_util::stream::StreamExt;
use lazy_static::lazy_static;
use pin_utils::pin_mut;
use std::cell::{RefCell};
use std::marker::Unpin;
use std::ops::{Deref, DerefMut};
use std::prelude::v1::*;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::thread::{self, Thread};

/// A single-threaded task pool for polling futures to completion.
///
/// This executor allows you to multiplex any number of tasks onto a single
/// thread. It's appropriate to poll strictly I/O-bound futures that do very
/// little work in between I/O actions.
///
/// To get a handle to the pool that implements
/// [`Spawn`](futures_core::task::Spawn), use the
/// [`spawner()`](LocalPool::spawner) method. Because the executor is
/// single-threaded, it supports a special form of task spawning for non-`Send`
/// futures, via [`spawn_local_obj`](LocalSpawner::spawn_local_obj).
#[derive(Debug)]
pub struct LocalPool {
    pool: FuturesUnordered<LocalFutureObj<'static, ()>>,
    incoming: Rc<Incoming>,
}

/// A handle to a [`LocalPool`](LocalPool) that implements
/// [`Spawn`](futures_core::task::Spawn).
#[derive(Clone, Debug)]
pub struct LocalSpawner {
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

// Set up and run a basic single-threaded spawner loop, invoking `f` on each
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

    /// Get a clonable handle to the pool as a [`Spawn`].
    pub fn spawner(&self) -> LocalSpawner {
        LocalSpawner {
            incoming: Rc::downgrade(&self.incoming)
        }
    }

    /// Run all tasks in the pool to completion.
    ///
    /// The given spawner, `spawn`, is used as the default spawner for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its spawner handle:
    ///
    /// ```
    /// use futures::executor::LocalPool;
    ///
    /// let mut pool = LocalPool::new();
    ///
    /// // ... spawn some initial tasks using `spawn.spawn()` or `spawn.spawn_local()`
    ///
    /// // run *all* tasks in the pool to completion, including any newly-spawned ones.
    /// pool.run();
    /// ```
    ///
    /// The function will block the calling thread until *all* tasks in the pool
    /// are complete, including any spawned while running existing tasks.
    pub fn run(&mut self) {
        run_executor(|local_waker| self.poll_pool(local_waker))
    }

    /// Runs all the tasks in the pool until the given future completes.
    ///
    /// The given spawner, `spawn`, is used as the default spawner for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its spawner handle:
    ///
    /// ```
    /// #![feature(pin, arbitrary_self_types, futures_api)]
    /// use futures::executor::LocalPool;
    /// use futures::future::ready;
    ///
    /// let mut pool = LocalPool::new();
    /// # let my_app  = ready(());
    ///
    /// // run tasks in the pool until `my_app` completes, by default spawning
    /// // further tasks back onto the pool
    /// pool.run_until(my_app);
    /// ```
    ///
    /// The function will block the calling thread *only* until the future `f`
    /// completes; there may still be incomplete tasks in the pool, which will
    /// be inert after the call completes, but can continue with further use of
    /// `run` or `run_until`. While the function is running, however, all tasks
    /// in the pool will try to make progress.
    pub fn run_until<F: Future>(&mut self, future: F) -> F::Output {
        pin_mut!(future);

        run_executor(|local_waker| {
            {
                // if our main task is done, so are we
                let result = future.as_mut().poll(local_waker);
                if let Poll::Ready(output) = result {
                    return Poll::Ready(output);
                }
            }

            self.poll_pool(local_waker);
            Poll::Pending
        })
    }

    // Make maximal progress on the entire pool of spawned task, returning `Ready`
    // if the pool is empty and `Pending` if no further progress can be made.
    fn poll_pool(&mut self, local_waker: &LocalWaker) -> Poll<()> {
        // state for the FuturesUnordered, which will never be used
        loop {
            // empty the incoming queue of newly-spawned tasks
            {
                let mut incoming = self.incoming.borrow_mut();
                for task in incoming.drain(..) {
                    self.pool.push(task)
                }
            }

            let ret = self.pool.poll_next_unpin(local_waker);
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

impl Default for LocalPool {
    fn default() -> Self {
        Self::new()
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
/// The default spawner for the future is a global `ThreadPool`.
///
/// Use a [`LocalPool`](LocalPool) if you need finer-grained control over
/// spawned tasks.
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut pool = LocalPool::new();
    pool.run_until(f)
}

/// Turn a stream into a blocking iterator.
///
/// When `next` is called on the resulting `BlockingStream`, the caller
/// will be blocked until the next element of the `Stream` becomes available.
/// The default spawner for the future is a global `ThreadPool`.
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
        LocalPool::new().run_until(self.stream.next())
    }
}

impl Spawn for LocalSpawner {
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), SpawnError> {
        if let Some(incoming) = self.incoming.upgrade() {
            incoming.borrow_mut().push(future.into());
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }

    fn status(&self) -> Result<(), SpawnError> {
        if self.incoming.upgrade().is_some() {
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }
}

impl LocalSpawn for LocalSpawner {
    fn spawn_local_obj(
        &mut self,
        future: LocalFutureObj<'static, ()>,
    ) -> Result<(), SpawnError> {
        if let Some(incoming) = self.incoming.upgrade() {
            incoming.borrow_mut().push(future);
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }
}
