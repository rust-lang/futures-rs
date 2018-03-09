use std::prelude::v1::*;

use std::cell::{RefCell};
use std::rc::{Rc, Weak};

use futures_core::{Future, Poll, Async, Stream};
use futures_core::task::{Context, Waker, LocalMap};
use futures_core::executor::{Executor, SpawnError};
use futures_core::never::Never;
use futures_util::stream::FuturesUnordered;

use thread::ThreadNotify;
use enter;

struct Task {
    fut: Box<Future<Item = (), Error = Never>>,
    map: LocalMap,
}

/// A single-threaded task pool.
///
/// This executor allows you to multiplex any number of tasks onto a single
/// thread. It's appropriate for strictly I/O-bound tasks that do very little
/// work in between I/O actions.
///
/// To get a handle to the pool that implements
/// [`Executor`](::futures_core::executor::Executor), use the
/// [`executor()`](LocalPool::executor) method. Because the executor is
/// single-threaded, it supports a special form of task spawning for non-`Send`
/// futures, via [`spawn_local`](LocalExecutor::spawn_local).
pub struct LocalPool {
    pool: FuturesUnordered<Task>,
    incoming: Rc<Incoming>,
}

/// A handle to a [`LocalPool`](LocalPool) that implements
/// [`Executor`](::futures_core::executor::Executor).
pub struct LocalExecutor {
    incoming: Weak<Incoming>,
}

type Incoming = RefCell<Vec<Task>>;

// Set up and run a basic single-threaded executor loop, invocing `f` on each
// turn.
fn run_executor<T, F: FnMut(&Waker) -> Async<T>>(mut f: F) -> T {
    let _enter = enter()
        .expect("cannot execute `LocalPool` executor from within \
                 another executor");

    ThreadNotify::with_current(|thread| {
        let waker = &Waker::from(thread.clone());
        loop {
            if let Async::Ready(t) = f(waker) {
                return t;
            }
            thread.park();
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
    pub fn run(&mut self, exec: &mut Executor) {
        run_executor(|waker| self.poll_pool(waker, exec))
    }

    /// Runs all the tasks in the pool until the given future completes.
    ///
    /// The given executor, `exec`, is used as the default executor for any
    /// *newly*-spawned tasks. You can route these additional tasks back into
    /// the `LocalPool` by using its executor handle:
    ///
    /// ```
    /// # extern crate futures;
    /// # use futures::executor::LocalPool;
    /// # use futures::future::{Future, ok};
    ///
    /// # fn main() {
    /// let mut pool = LocalPool::new();
    /// let mut exec = pool.executor();
    /// # let my_app: Box<Future<Item = (), Error = ()>> = Box::new(ok(()));
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
    pub fn run_until<F>(&mut self, mut f: F, exec: &mut Executor) -> Result<F::Item, F::Error>
        where F: Future
    {
        // persistent state for the "main task"
        let mut main_map = LocalMap::new();

        run_executor(|waker| {
            {
                let mut main_cx = Context::new(&mut main_map, waker, exec);

                // if our main task is done, so are we
                match f.poll(&mut main_cx) {
                    Ok(Async::Ready(v)) => return Async::Ready(Ok(v)),
                    Err(err) => return Async::Ready(Err(err)),
                    _ => {}
                }
            }

            self.poll_pool(waker, exec);
            Async::Pending
        })
    }

    // Make maximal progress on the entire pool of spawned task, returning `Ready`
    // if the pool is empty and `Pending` if no further progress can be made.
    fn poll_pool(&mut self, waker: &Waker, exec: &mut Executor) -> Async<()> {
        // state for the FuturesUnordered, which will never be used
        let mut pool_map = LocalMap::new();
        let mut pool_cx = Context::new(&mut pool_map, waker, exec);

        loop {
            // empty the incoming queue of newly-spawned tasks
            {
                let mut incoming = self.incoming.borrow_mut();
                for task in incoming.drain(..) {
                    self.pool.push(task)
                }
            }

            if let Ok(ret) = self.pool.poll_next(&mut pool_cx) {
                // we queued up some new tasks; add them and poll again
                if !self.incoming.borrow().is_empty() {
                    continue;
                }

                // no queued tasks; we may be done
                match ret {
                    Async::Pending => return Async::Pending,
                    Async::Ready(None) => return Async::Ready(()),
                    _ => {}
                }
            }
        }
    }
}

/// Run a future to completion on the current thread.
///
/// This function will block the caller until the given future has completed.
/// Any tasks spawned onto the default executor will *also* be run on the
/// current thread, **but they may not complete before the function
/// returns**. Instead, once the starting future has completed, these other
/// tasks are simply dropped.
///
/// Use a [`LocalPool`](LocalPool) if you need finer-grained control over
/// spawned tasks.
pub fn block_on<F: Future>(f: F) -> Result<F::Item, F::Error> {
    let mut pool = LocalPool::new();
    let mut exec = pool.executor();

    // run our main future to completion
    let res = pool.run_until(f, &mut exec);
    // run any remainingspawned tasks to completion
    pool.run(&mut exec);

    res
}

impl Executor for LocalExecutor {
    fn spawn(&mut self, f: Box<Future<Item = (), Error = Never> + Send>) -> Result<(), SpawnError> {
        self.spawn_task(Task {
            fut: f,
            map: LocalMap::new(),
        })
    }

    fn status(&self) -> Result<(), SpawnError> {
        if self.incoming.upgrade().is_some() {
            Ok(())
        } else {
            Err(SpawnError::shutdown())
        }
    }
}

impl LocalExecutor {
    fn spawn_task(&self, task: Task) -> Result<(), SpawnError> {
        let incoming = self.incoming.upgrade().ok_or(SpawnError::shutdown())?;
        incoming.borrow_mut().push(task);
        Ok(())
    }

    /// Spawn a non-`Send` future onto the associated [`LocalPool`](LocalPool).
    pub fn spawn_local<F>(&mut self, f: F) -> Result<(), SpawnError>
        where F: Future<Item = (), Error = Never> + 'static
    {
        self.spawn_task(Task {
            fut: Box::new(f),
            map: LocalMap::new(),
        })
    }
}

impl Future for Task {
    type Item = ();
    type Error = Never;

    fn poll(&mut self, cx: &mut Context) -> Poll<(), Never> {
        self.fut.poll(&mut cx.with_locals(&mut self.map))
    }
}
