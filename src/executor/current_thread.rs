//! Execute tasks on the current thread

use Async;
use executor::{self, Spawn};
use future::{self, Future, Executor, ExecuteError, ExecuteErrorKind};
use scheduler;
use task_impl::ThreadNotify;

use std::prelude::v1::*;

use std::{fmt, ptr, thread};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// Executes tasks on the current thread.
#[derive(Debug, Clone)]
pub struct CurrentThread {
    // Prevent the handle from moving across threads.
    _p: ::std::marker::PhantomData<Rc<()>>,
}

/// Executes dameonized tasks on the current thread.
#[derive(Debug, Clone)]
pub struct DaemonExecutor {
    // Prevent the handle from moving across threads.
    _p: ::std::marker::PhantomData<Rc<()>>,
}

// An object for cooperatively executing multiple tasks on a single thread.
// Useful for working with non-`Send` futures.
//
// NB: this is not `Send`
#[derive(Debug)]
struct TaskRunner {
    /// Number of non-daemon tasks being executed by this `TaskRunner`.
    non_daemons: usize,

    /// Executes futures.
    scheduler: Scheduler,
}

#[derive(Debug)]
struct CurrentRunner {
    /// When set to true, the executor should return immediately, even if there
    /// still are non-daemon tasks to run.
    cancel: Cell<bool>,

    /// Raw pointer to the current scheduler.
    ///
    /// The raw pointer is required in order to store it in a thread-local slot.
    scheduler: Cell<*mut Scheduler>,
}

type Scheduler = scheduler::Scheduler<SpawnedFuture, Arc<ThreadNotify>>;

#[derive(Debug)]
struct SpawnedFuture {
    /// True if the spawned future should not prevent the executor from
    /// terminating.
    daemon: bool,

    /// The task to execute.
    inner: Task,
}

struct Task(Spawn<Box<Future<Item = (), Error = ()>>>);

/// Current thread's task runner. This is set in `TaskRunner::with`
thread_local!(static CURRENT: CurrentRunner = CurrentRunner {
    cancel: Cell::new(false),
    scheduler: Cell::new(ptr::null_mut()),
});

impl CurrentThread {
    /// Returns a handle to the current `CurrentThread` executor.
    pub fn current() -> CurrentThread {
        CurrentThread {
            _p: ::std::marker::PhantomData,
        }
    }

    /// Returns a new handle that spawns daemonized tasks on the current thread.
    pub fn daemon_executor(&self) -> DaemonExecutor {
        DaemonExecutor {
            _p: ::std::marker::PhantomData,
        }
    }

    /// Execute the given future *synchronously* on the current thread, blocking
    /// until it (and all spawned tasks) completes and returning its result.
    ///
    /// In more detail, this function blocks until:
    ///
    /// - the given future completes, *and*
    /// - all spawned tasks complete, or `cancel_all_spawned` is invoked
    ///
    /// Note that there is no `'static` or `Send` requirement on the future.
    pub fn block_on_all<F: Future>(future: F) -> Result<F::Item, F::Error> {
        TaskRunner::enter(|| future)
    }

    /// Execute the given closure, then block until all spawned tasks complete.
    ///
    /// In more detail, this function will block until:
    /// - All spawned tasks are complete, or
    /// - `cancel_all_spawned` is invoked.
    pub fn block_with_init<F>(f: F) where F: FnOnce() {
        drop(TaskRunner::enter(|| {
            f();
            future::ok::<(), ()>(())
        }))
    }

    /// Spawns a task, i.e. one that must be explicitly either
    /// blocked on or killed off before `block_*` will return.
    ///
    /// # Panics
    ///
    /// This function can only be invoked within a future given to a `block_*`
    /// invocation; any other use will result in a panic.
    pub fn spawn<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
    {
        spawn(future, false).unwrap_or_else(|_| {
            panic!("cannot call `spawn` unless your thread is already \
                    in the context of a call to `block_on_all` or \
                    `block_with_init`")
        })
    }

    /// Spawns a daemon, which does *not* block the pending `block_on_all` call.
    ///
    /// # Panics
    ///
    /// This function can only be invoked within a future given to a `block_*`
    /// invocation; any other use will result in a panic.
    pub fn spawn_daemon<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
    {
        spawn(future, true).unwrap_or_else(|_| {
            panic!("cannot call `spawn_daemon` unless your thread is already \
                    in the context of a call to `block_on_all` or \
                    `block_with_init`")
        })
    }

    /// Cancels *all* spawned tasks and daemons.
    ///
    /// # Panics
    ///
    /// This function can only be invoked within a future given to a `block_*`
    /// invocation; any other use will result in a panic.
    pub fn cancel_all_spawned() {
        CurrentRunner::with(|runner| runner.cancel_all_spawned())
            .unwrap_or_else(|()| {
                panic!("cannot call `cancel_all_spawned` unless your thread is \
                        already in the context of a call to `block_on_all` or \
                        `block_with_init`")
            })
    }
}

impl<F> Executor<F> for CurrentThread
where F: Future<Item = (), Error = ()> + 'static
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        spawn(future, false)
    }
}


impl<F> Executor<F> for DaemonExecutor
where F: Future<Item = (), Error = ()> + 'static
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        spawn(future, true)
    }
}

fn spawn<F>(future: F, daemon: bool) -> Result<(), ExecuteError<F>>
where F: Future<Item = (), Error = ()> + 'static,
{
    CURRENT.with(|current| {
        if current.scheduler.get().is_null() {
            Err(ExecuteError::new(ExecuteErrorKind::Shutdown, future))
        } else {
            let spawned = SpawnedFuture {
                daemon: daemon,
                inner: Task::new(future),
            };

            unsafe {
                (*current.scheduler.get()).push(spawned);
            }

            Ok(())
        }
    })
}

impl TaskRunner {
    /// Return a new `TaskRunner`
    fn new(thread_notify: Arc<ThreadNotify>) -> TaskRunner {
        TaskRunner {
            non_daemons: 0,
            scheduler: scheduler::Scheduler::new(thread_notify),
        }
    }

    /// Enter a new `TaskRunner` context
    fn enter<F, A>(f: F) -> Result<A::Item, A::Error>
        where F: FnOnce() -> A,
              A: Future,
    {
        // Create a new task runner that will be used for the duration of `f`.
        ThreadNotify::with_current(|thread_notify| {
            // The runner has to be created outside of the MY_TASK_RUNNER.with
            // block.
            let mut runner = TaskRunner::new(thread_notify.clone());

            CURRENT.with(|current| {
                // Make sure that another task runner is not set.
                assert!(current.scheduler.get().is_null());

                // Set the scheduler to the TLS and perform setup work,
                // returning a future to execute.
                //
                // This could possibly spawn other tasks.
                let future = current.set_scheduler(&mut runner.scheduler, f);

                // Execute the runner
                runner.finish(thread_notify, current, future)
            })
        })
    }

    fn finish<F: Future>(&mut self,
                         thread_notify: &Arc<ThreadNotify>,
                         current: &CurrentRunner,
                         future: F)
        -> Result<F::Item, F::Error>
    {
        let mut result = None;
        let mut future = Some(executor::spawn(future));

        while future.is_some() || self.non_daemons > 0 {
            if current.cancel.get() {
                // TODO: This probably can be improved
                current.cancel.set(false);

                debug_assert!(current.scheduler.get().is_null());
                self.scheduler = scheduler::Scheduler::new(thread_notify.clone());
            }

            let res = future.as_mut()
                .map(|f| f.poll_future_notify(thread_notify, 0));

            match res {
                Some(Ok(Async::Ready(e))) => {
                    result = Some(Ok(e));
                    future = None;
                }
                Some(Err(e)) => {
                    result = Some(Err(e));
                    future = None;
                }
                Some(Ok(Async::NotReady)) |
                None => {}
            }

            self.poll_all(current);

            if future.is_some() || self.non_daemons > 0 {
                thread_notify.park();
            }
        }

        result.unwrap()
    }

    fn poll_all(&mut self, current: &CurrentRunner) {
        use scheduler::Tick;

        loop {
            let res = self.scheduler.tick(|scheduler, spawned, notify| {
                current.set_scheduler(scheduler, || {
                    match spawned.inner.0.poll_future_notify(notify, 0) {
                        Ok(Async::Ready(_)) => Async::Ready(()),
                        Ok(Async::NotReady) => Async::NotReady,
                        Err(_) => Async::Ready(()),
                    }
                })
            });

            match res {
                Tick::Data(_) => {},
                Tick::Empty => {
                    return;
                }
                Tick::Inconsistent => {
                    // Yield the thread and loop
                    thread::yield_now();
                }
            }
        }
    }
}

impl CurrentRunner {
    fn with<F, R>(f: F) -> Result<R, ()>
    where F: FnOnce(&Self) -> R,
    {
        CURRENT.with(|current| {
            if current.scheduler.get().is_null() {
                Err(())
            } else {
                Ok(f(current))
            }
        })
    }

    /// Set the provided scheduler to the TLS slot for the duration of the
    /// closure
    fn set_scheduler<F, R>(&self, scheduler: &mut Scheduler, f: F) -> R
    where F: FnOnce() -> R
    {
        // Ensure that the runner is removed from the thread-local context
        // when leaving the scope. This handles cases that involve panicking.
        struct Reset<'a>(&'a CurrentRunner);

        impl<'a> Drop for Reset<'a> {
            fn drop(&mut self) {
                self.0.scheduler.set(ptr::null_mut());
            }
        }

        let _reset = Reset(self);

        self.scheduler.set(scheduler as *mut Scheduler);

        f()
    }

    fn cancel_all_spawned(&self) {
        self.cancel.set(true);
    }
}

impl Task {
    fn new<T: Future<Item = (), Error = ()> + 'static>(f: T) -> Self {
        Task(executor::spawn(Box::new(f)))
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Task")
            .finish()
    }
}
