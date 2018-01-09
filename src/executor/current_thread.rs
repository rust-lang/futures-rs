//! Execute tasks on the current thread
//!
//! [`CurrentThread`] provides an executor that keeps futures on the same thread
//! that they are submitted on. This allows it to execute futures that are
//! `!Send`. For more details on general executor concepts, like executing
//! futures, see [here].
//!
//! Before being able to execute futures onto [`CurrentThread`], an executor
//! context must be setup. This is done by calling [`run`].  From within that
//! context, [`CurrentThread::execute`] may be called with the future to run in
//! the background.
//!
//! ```
//! use futures::executor::CurrentThread;
//! use futures::future::{self, lazy};
//!
//! // Calling execute here results in a panic
//! // CurrentThread::execute(my_future);
//!
//! let ret = CurrentThread::run(|_| {
//!     // The execution context is setup, futures may be executed.
//!     CurrentThread::execute(lazy(|| {
//!         println!("called from the current thread executor");
//!         Ok(())
//!     }));
//!
//!     // this initialization closure also returns a future to be run to
//!     // completion
//!     future::ok::<_, ()>(3)
//! }).unwrap();
//!
//! assert_eq!(ret, 3);
//! ```
//!
//! # Execution model
//!
//! When a [`CurrentThread`] execution context is setup with `run`,
//! the current thread will block and all the futures managed by the executor
//! are driven to completion. Whenever a future receives a notification, it is
//! pushed to the end of a scheduled list. The [`CurrentThread`] executor will
//! drain this list, advancing the state of each future.
//!
//! All futures managed by [`CurrentThread`] will remain on the current thread,
//! as such, [`CurrentThread`] is able to safely execute futures that are `!Send`.
//!
//! Once a future is complete, it is dropped. Once all [non
//! daemon](#daemon-futures) futures are completed, [`CurrentThread`] unblocks.
//!
//! [`CurrentThread`] makes a best effort to fairly schedule futures that it
//! manages.
//!
//! # Daemon Futures
//!
//! A daemon future is a future that does not require to be complete in order
//! for [`CurrentThread`] to complete running. These are useful for background
//! "maintenance" tasks that are not critical to the completion of the primary
//! computation.
//!
//! When [`CurrentThread`] completes running and unblocks, any daemon futures
//! that have not yet completed are immediately dropped.
//!
//! A daemon future can be executed with [`CurrentThread::execute_daemon`].
//!
//! [here]: https://tokio.rs/docs/going-deeper-futures/tasks/
//! [`CurrentThread`]: struct.CurrentThread.html
//! [`run`]: struct.CurrentThread.html#method.run
//! [`CurrentThread::execute`]: struct.CurrentThread.html#method.execute
//! [`CurrentThread::execute_daemon`]: struct.CurrentThread.html#method.execute_daemon

use executor::task_runner::{set_current, with_current};
use executor::{self, Sleep, TaskRunner, Wakeup, Notify};
use future::{Executor, ExecuteError, ExecuteErrorKind};
use prelude::*;
use task_impl::ThreadNotify;

use std::prelude::v1::*;

use std::rc::Rc;
use std::sync::Arc;

/// Executes futures on the current thread.
///
/// All futures executed using this executor will be executed on the current
/// thread as non-daemon futures. As such, [`CurrentThread`] will wait for these
/// futures to complete before returning from `run`.
///
/// For more details, see the [module level](index.html) documentation.
#[derive(Debug, Clone)]
pub struct CurrentThread {
    // Prevent the handle from moving across threads.
    _p: ::std::marker::PhantomData<Rc<()>>,
}

/// Executes daemon futures on the current thread.
///
/// All futures executed using this executor will be executed on the current
/// thread as daemon futures. As such, [`CurrentThread`] will **not** wait for
/// these futures to complete before returning from `run`.
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
    enter: &'a executor::Enter,
}

impl CurrentThread {
    /// Returns an executor that executes futures on the current thread.
    ///
    /// This executor can be moved across threads. Futures submitted for
    /// execution will be executed on the same thread that they were submitted
    /// on.
    ///
    /// The user of `CurrentThread` must ensure that when a future is submitted
    /// to the executor, that it is done from the context of a `run` call.
    ///
    /// For more details, see the [module level](index.html) documentation.
    pub fn current() -> CurrentThread {
        CurrentThread {
            _p: ::std::marker::PhantomData,
        }
    }

    /// Returns an executor that executes daemon futures on the current thread.
    ///
    /// This executor can be moved across threads. Futures submitted for
    /// execution will be executed on the same thread that they were submitted
    /// on.
    ///
    /// The user of `CurrentThread` must ensure that when a future is submitted
    /// to the executor, that it is done from the context of a `run` call.
    ///
    /// For more details, see the [module level](index.html) documentation.
    pub fn daemon_executor(&self) -> DaemonExecutor {
        DaemonExecutor {
            _p: ::std::marker::PhantomData,
        }
    }

    /// Calls the given closure, then block until all futures submitted for
    /// execution complete.
    ///
    /// In more detail, this function will block until:
    ///
    /// - All executing futures are complete, or
    /// - `cancel_all_executing` is invoked.
    ///
    /// The closure provided receives a `Context` argument which can be used to
    /// extract the `Enter` handle that this invocation will be running with.
    /// This can be useful for interoperation with other libraries which
    /// require a reference to the `Enter` handle, for example.
    ///
    /// # Panics
    ///
    /// This function will internally call `executor::enter` and panic if that
    /// call fails. This means that it is illegal to call this function
    /// recursively with any other executor-like interfaces. For example this
    /// will immediately panic:
    ///
    /// ```no_run
    /// use futures::future;
    /// use futures::executor::CurrentThread;
    ///
    /// CurrentThread::run(|_| {
    ///     CurrentThread::run(|_| {
    ///         // never executed
    ///         future::ok::<(), ()>(())
    ///     });
    ///
    ///     // also never executed
    ///     future::ok::<(), ()>(())
    /// });
    /// ```
    ///
    /// This function cannot be called recursively with the following (but not
    /// included to) functions:
    ///
    /// * `CurrentThread::run`
    /// * `CurrentThread::run_with_sleep`
    /// * `TaskRunner::poll`
    /// * `TaskExecutor::with_current_thread_using_me`
    ///
    /// These executor-like interfaces are not intended to be used recursively
    /// and instead it's recommended to only have one per thread. For example a
    /// thread might *once* call `CurrentThread::run` (but never recursively)
    /// while other threads could use other executors if necessary.
    pub fn run<F, R>(f: F) -> Result<R::Item, R::Error>
    where F: FnOnce(&mut Context) -> R,
          R: IntoFuture,
    {
        ThreadNotify::with_current(|mut thread_notify| {
            CurrentThread::run_with_sleep(&mut thread_notify, f)
        })
    }

    /// Calls the given closure with a custom sleep strategy.
    ///
    /// This function is the same as `run` except that it allows customizing the
    /// sleep strategy.
    ///
    /// # Panics
    ///
    /// This function will panic if used recursively with any other
    /// executor-like interface. For more detail on the sources of panics
    /// see the `CurrentThread::run` documentation.
    pub fn run_with_sleep<S, F, R>(sleep: &mut S, f: F) -> Result<R::Item, R::Error>
    where F: FnOnce(&mut Context) -> R,
          R: IntoFuture,
          S: Sleep,
    {
        let mut runner = TaskRunner::new(sleep.wakeup());
        let notify = Arc::new(Wakeup2Notify(sleep.wakeup()));

        // Kick off any initial work through the callback provided
        let future = set_current(&runner.executor(), |enter| {
            f(&mut Context {
                enter: &enter,
            }).into_future()
        });

        // So long as there's pending work we keep polling and sleeping.
        let mut ret = None;
        let mut future = executor::spawn(future);
        loop {
            runner.poll();
            if ret.is_none() {
                match future.poll_future_notify(&notify, 0) {
                    Ok(Async::NotReady) => {}
                    Ok(Async::Ready(e)) => ret = Some(Ok(e)),
                    Err(e) => ret = Some(Err(e)),
                }
            }
            if runner.is_done() {
                if let Some(ret) = ret {
                    return ret
                }
            }
            sleep.sleep();
        }

        struct Wakeup2Notify<T>(T);

        impl<T: Wakeup> Notify for Wakeup2Notify<T> {
            fn notify(&self, _id: usize) {
                self.0.wakeup()
            }
        }
    }

    /// Executes a future on the current thread.
    ///
    /// The provided future must complete or be canceled before
    /// `run` will return.
    ///
    /// # Panics
    ///
    /// This function can only be invoked from the context of a
    /// `run` call; any other use will result in a panic.
    pub fn execute<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
    {
        execute(future, false).unwrap_or_else(|_| {
            panic!("cannot call `execute` unless the thread is already \
                    in the context of a call to `run`")
        })
    }

    /// Executes a daemon future on the current thread.
    ///
    /// Completion of the provided future is not required for the pending
    /// `run` call to complete. If `run` returns before `future` completes, it
    /// will be dropped.
    ///
    /// # Panics
    ///
    /// This function can only be invoked from the context of a
    /// `run` call; any other use will result in a panic.
    pub fn execute_daemon<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
    {
        execute(future, true).unwrap_or_else(|_| {
            panic!("cannot call `execute` unless the thread is already \
                    in the context of a call to `run`")
        })
    }

    /// Cancels *all* executing futures.
    ///
    /// This cancels both daemon and non-daemon futures.
    ///
    /// # Panics
    ///
    /// This function can only be invoked from the context of a
    /// `run` call; any other use will result in a panic.
    pub fn cancel_all_executing() {
        with_current(|current| {
            match current {
                Some(c) => c.cancel_all_executing(),
                None => {
                    panic!("cannot call `cancel_all_executing` unless the \
                            thread is already in the context of a call to `run`")
                }
            }
        })
    }
}

impl<F> Executor<F> for CurrentThread
where F: Future<Item = (), Error = ()> + 'static
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        execute(future, false)
    }
}

impl<'a> Context<'a> {
    /// Returns a reference to the executor `Enter` handle.
    pub fn enter(&self) -> &executor::Enter {
        &self.enter
    }
}

/// Submits a future to the current `CurrentThread` executor. This is done by
/// checking the thread-local variable tracking the current executor.
///
/// If this function is not called in context of an executor, i.e. outside of
/// `run`, then `Err` is returned.
///
/// This function does not panic.
fn execute<F>(future: F, daemon: bool) -> Result<(), ExecuteError<F>>
where F: Future<Item = (), Error = ()> + 'static,
{
    with_current(|current| {
        match current {
            Some(c) if daemon => c.execute_daemon(future),
            Some(c) => c.execute(future),
            None => Err(ExecuteError::new(ExecuteErrorKind::Shutdown, future))
        }
    })
}
