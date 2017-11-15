//! Execute tasks on the current thread
//!
//! [`CurrentThread`] provides an executor that keeps spawned futures keeps
//! spawned futures on the same thread that they are spawned on. This allows it
//! to execute futures that are `!Send`. For more details on general executor
//! concepts, like spawning tasks. See [here].
//!
//! Before being able to spawn futures onto [`CurrentThread`], an executor
//! context must be setup. This is done by calling either [`block_with_init`].
//! From within that context, [`CurrentThread::spawn`] may be called with the
//! future to run in the background.
//!
//! ```
//! # use futures::executor::current_thread::*;
//! use futures::future::lazy;
//!
//! // Calling spawn here results in a panic
//! // CurrentThread::spawn(my_future);
//!
//! CurrentThread::block_with_init(|_| {
//!     // The execution context is setup, futures may be spawned.
//!     CurrentThread::spawn(lazy(|| {
//!         println!("called from the current thread executor");
//!         Ok(())
//!     }));
//! });
//! ```
//!
//! # Execution model
//!
//! When a [`CurrentThread`] execution context is setup with `block_with_init`,
//! the current thread will block and all the tasks managed by the executor are
//! driven to completion. Whenever a task receives a notification, it is pushed
//! to the end of a scheduled list. The [`CurrentThread`] executor will drain
//! this list, advancing the state of each future.
//!
//! All futures managed by [`CurrentThread`] will remain on the current thread,
//! as such, [`CurrentThread`] is able to safely spawn futures that are `!Send`.
//!
//! Once a task is complete, it is dropped. Once all [non daemon](#daemon-tasks) tasks are
//! completed, [`CurrentThread`] unblocks.
//!
//! [`CurrentThread`] makes a best effort to fairly schedule tasks that it
//! manages.
//!
//! # Daemon tasks
//!
//! A daemon task is a task that does not require to be complete in order for
//! [`CurrentThread`] to complete running. These are useful for background
//! "maintenance" tasks that are not critical to the completion of the primary
//! computation.
//!
//! When [`CurrentThread`] completes running and unblocks, any daemon tasks that
//! have not yet completed are immediately dropped.
//!
//! A daemon task can be spawned with [`CurrentThread::spawn_daemon`].
//!
//! [here]: https://tokio.rs/docs/going-deeper-futures/tasks/
//! [`CurrentThread`]: struct.CurrentThread.html
//! [`block_with_init`]: struct.CurrentThread.html#method.block_with_init
//! [`CurrentThread::spawn`]: struct.CurrentThread.html#method.spawn
//! [`CurrentThread::spawn_daemon`]: struct.CurrentThread.html#method.spawn_daemon

use Async;
use executor::{self, Spawn};
use future::{Future, Executor, ExecuteError, ExecuteErrorKind};
use scheduler;
use task_impl::ThreadNotify;

use std::prelude::v1::*;

use std::{fmt, ptr, thread};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// Executes tasks on the current thread.
///
/// All tasks spawned using this executor will be executed on the current thread
/// as non-daemon tasks. As such, [`CurrentThread`] will wait for these tasks to
/// complete before returning from `block_with_init`.
///
/// For more details, see the [module level](index.html) documentation.
#[derive(Debug, Clone)]
pub struct CurrentThread {
    // Prevent the handle from moving across threads.
    _p: ::std::marker::PhantomData<Rc<()>>,
}

/// Executes dameonized tasks on the current thread.
///
/// All tasks spawned using this executor will be executed on the current thread
/// as daemon tasks. As such, [`CurrentThread`] will **not** wait for these tasks to
/// complete before returning from `block_with_init`.
///
/// For more details, see the [module level](index.html) documentation.
#[derive(Debug, Clone)]
pub struct DaemonExecutor {
    // Prevent the handle from moving across threads.
    _p: ::std::marker::PhantomData<Rc<()>>,
}

/// Provides execution context
///
/// This currently does not do anything, but allows future improvements to be
/// made in a backwards compatible way.
#[derive(Debug)]
pub struct Context<'a> {
    _p: ::std::marker::PhantomData<&'a ()>,
}

/// Implements the "blocking" logic for the current thread executor. A
/// `TaskRunner` will be created during `block_with_init` and will sit on the
/// stack until execution is complete.
#[derive(Debug)]
struct TaskRunner {
    /// Executes futures.
    scheduler: Scheduler,
}

#[derive(Debug)]
struct CurrentRunner {
    /// When set to true, the executor should return immediately, even if there
    /// still are non-daemon tasks to run.
    cancel: Cell<bool>,

    /// Number of non-daemon tasks currently being executed by the runner.
    non_daemons: Cell<usize>,

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
    non_daemons: Cell::new(0),
    scheduler: Cell::new(ptr::null_mut()),
});

impl CurrentThread {
    /// Returns an executor that executes tasks on the current thread.
    ///
    /// This executor can be moved across threads. Spawned tasks will be executed
    /// on the same thread that spawn was called on.
    ///
    /// The user of `CurrentThread` must ensure that when a future is submitted
    /// to the executor, that it is done from the context of a `block_with_init`
    /// call.
    ///
    /// For more details, see the [module level](index.html) documentation.
    pub fn current() -> CurrentThread {
        CurrentThread {
            _p: ::std::marker::PhantomData,
        }
    }

    /// Returns an executor that spawns daemon tasks on the current thread.
    ///
    /// This executor can be moved across threads. Spawned tasks will be
    /// executed on the same thread that spawn was called on.
    ///
    /// The user of `CurrentThread` must ensure that when a future is submitted
    /// to the executor, that it is done from the context of a `block_with_init`
    /// call.
    ///
    /// For more details, see the [module level](index.html) documentation.
    pub fn daemon_executor(&self) -> DaemonExecutor {
        DaemonExecutor {
            _p: ::std::marker::PhantomData,
        }
    }

    /// Execute the given closure, then block until all spawned tasks complete.
    ///
    /// In more detail, this function will block until:
    /// - All spawned tasks are complete, or
    /// - `cancel_all_spawned` is invoked.
    pub fn block_with_init<F, R>(f: F) -> R
    where F: FnOnce(&mut Context) -> R
    {
        TaskRunner::enter(f)
    }

    /// Spawns a task, i.e. one that must be explicitly either
    /// blocked on or killed off before `block_with_init` will return.
    ///
    /// # Panics
    ///
    /// This function can only be invoked within a future given to a
    /// `block_with_init` invocation; any other use will result in a panic.
    pub fn spawn<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
    {
        spawn(future, false).unwrap_or_else(|_| {
            panic!("cannot call `spawn` unless your thread is already \
                    in the context of a call to `block_with_init`")
        })
    }

    /// Spawns a daemon, which does *not* block the pending `block_with_init` call.
    ///
    /// # Panics
    ///
    /// This function can only be invoked within a future given to a
    /// `block_with_init` invocation; any other use will result in a panic.
    pub fn spawn_daemon<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
    {
        spawn(future, true).unwrap_or_else(|_| {
            panic!("cannot call `spawn` unless your thread is already \
                    in the context of a call to `block_with_init`")
        })
    }

    /// Cancels *all* spawned tasks and daemons.
    ///
    /// # Panics
    ///
    /// This function can only be invoked within a future given to a
    /// `block_with_init` invocation; any other use will result in a panic.
    pub fn cancel_all_spawned() {
        CurrentRunner::with(|runner| runner.cancel_all_spawned())
            .unwrap_or_else(|()| {
                panic!("cannot call `spawn` unless your thread is already \
                        in the context of a call to `block_with_init`")
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

/// Spawns a future onto the current `CurrentThread` executor. This is done by
/// checking the thread-local variable tracking the current executor.
///
/// If this function is not called in context of an executor, i.e. outside of
/// `block_with_init`, then `Err` is returned.
///
/// This function does not panic.
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

            if !daemon {
                let non_daemons = current.non_daemons.get();
                current.non_daemons.set(non_daemons + 1);
            }

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
            scheduler: scheduler::Scheduler::new(thread_notify),
        }
    }

    /// Enter a new `TaskRunner` context
    ///
    /// This function handles advancing the scheduler state and blocking while
    /// listening for notified futures.
    ///
    /// First, a new task runner is created backed by the current `ThreadNotify`
    /// handle. Passing `ThreadNotify` into the scheduler is how scheduled
    /// futures unblock the thread, signalling that there is more work to do.
    ///
    /// Before any future is polled, the scheduler must be set to a thread-local
    /// variable so that `spawn` is able to spawn new tasks into the current
    /// executor. Because `Scheduler::push` requires `&mut self`, this
    /// introduces a mutability hazard. This hazard is minimized with some
    /// indirection. See `set_scheduler` for more details.
    ///
    /// Once all context is setup, the init closure is invoked. This is the
    /// "boostrapping" process that spawns the initial futures into the
    /// scheduler. After this, the function loops and advances the scheduler
    /// state until all non daemon futures complete. When no scheduled futures
    /// are ready to be advanced, the thread is blocked using
    /// `ThreadNotify::park`.
    fn enter<F, R>(f: F) -> R
    where F: FnOnce(&mut Context) -> R,
    {
        // Create a new task runner that will be used for the duration of `f`.
        ThreadNotify::with_current(|thread_notify| {
            let mut runner = TaskRunner::new(thread_notify.clone());

            CURRENT.with(|current| {
                // Make sure that another task runner is not set.
                //
                // This should not be ever possible due to how `set_scheduler`
                // is setup, but better safe than sorry!
                assert!(current.scheduler.get().is_null());

                // Set the scheduler to the TLS and perform setup work,
                // returning a future to execute.
                //
                // This could possibly spawn other tasks.
                let ret = current.set_scheduler(&mut runner.scheduler, || {
                    let mut ctx = Context { _p: ::std::marker::PhantomData };
                    f(&mut ctx)
                });

                // Execute the runner.
                //
                // This function will not return until either
                //
                // a) All non daemon tasks have completed execution
                // b) `cancel_all_spawned` is called, forcing the executor to
                // return.
                runner.run(thread_notify, current);

                ret
            })
        })
    }

    fn run(&mut self, thread_notify: &Arc<ThreadNotify>, current: &CurrentRunner) {
        loop {
            // Check if cancel was called
            if current.cancel.get() {
                // TODO: This probably can be improved
                current.cancel.set(false);

                // This is an extra sanity check to ensure that a pointer didn't
                // "leak" out of `set_scheduler`. `run` is *not* called from
                // within the context of `set_scheduler`, so the TLS should not
                // have a value.
                debug_assert!(current.scheduler.get().is_null());

                self.scheduler = scheduler::Scheduler::new(thread_notify.clone());
            }

            // Poll all scheduled futures. These are futures managed by the
            // scheduler that have received a readiness notification.
            self.poll_all(current);

            // If all non-daemon tasks have completed, exit the run loop.
            if current.non_daemons.get() == 0 {
                break;
            }

            // Block the current thread until a future managed by the scheduler
            // receives a readiness notification.
            thread_notify.park();
        }
    }

    fn poll_all(&mut self, current: &CurrentRunner) {
        use scheduler::Tick;

        loop {
            let res = self.scheduler.tick(|scheduler, spawned, notify| {
                // `scheduler` is a `&mut Scheduler` reference returned back
                // from the scheduler to us, but only within the context of this
                // closure.
                //
                // This lets us push new futures into the scheduler. It also
                // lets us pass the scheduler mutable reference into
                // `set_scheduler`, which sets the thread-local variable that
                // `CurrentThread::spawn` uses for spawning new tasks into the
                // "current" executor.
                //
                // See `set_scheduler` documentation for more details on how we
                // guard against mutable pointer aliasing.
                current.set_scheduler(scheduler, || {
                    match spawned.inner.0.poll_future_notify(notify, 0) {
                        Ok(Async::Ready(_)) | Err(_) => {
                            Async::Ready(spawned.daemon)
                        }
                        Ok(Async::NotReady) => Async::NotReady,
                    }
                })
            });

            match res {
                Tick::Data(is_daemon) => {
                    if !is_daemon {
                        let non_daemons = current.non_daemons.get();
                        debug_assert!(non_daemons > 0);
                        current.non_daemons.set(non_daemons - 1);
                    }
                },
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
    /// closure.
    ///
    /// `CurrentThread::spawn` will access the CURRENT thread-local variable in
    /// order to push a future into the scheduler. This requires a `&mut`
    /// reference, introducing mutability hazards.
    ///
    /// Rust requires that `&mut` references are not aliases, i.e. there are
    /// never two "live" mutable references to the same piece of data. In order
    /// to store a `&mut` reference in a thread-local variable, we must ensure
    /// that one can not access the scheduler anywhere else.
    ///
    /// To do this, we only allow access to the thread local variable from
    /// within the closure passed to `set_scheduler`. This function also takes a
    /// &mut reference to the scheduler, which is essentially holding a "lock"
    /// on that reference, preventing any other location in the code from
    /// also getting that &mut reference.
    ///
    /// When `set_scheduler` returns, the thread-local variable containing the
    /// mut reference is set to null. This is done even if the closure panics.
    ///
    /// This reduces the odds of introducing pointer aliasing.
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
