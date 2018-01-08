use std::prelude::v1::*;

use std::cell::{Cell, RefCell};
use std::fmt;
use std::ptr;
use std::rc::{Rc, Weak};

use prelude::*;
use executor::{self, Spawn, Wakeup, Enter};
use future::{Executor, ExecuteError, ExecuteErrorKind};
use scheduler::{Scheduler, Schedule, Tick};

/// An object for cooperatively scheduling multiple futures on one thread.
///
/// A `TaskRunner` provides two implementors of the `Executor` trait, allowing
/// futures to be spawned into a `TaskRunner` and scheduled cooperatively one
/// another on the same thread. The `CurrentThread` type, for example,
/// internally uses a `TaskRunner` for running futures.
///
/// A `TaskRunner` is *not* a `Send` structure. Once created it cannot move
/// across threads, nor can its executor handle (`TaskExecutor`).
///
/// Note that `TaskRunner` is likely a low-level detail you shouldn't concern
/// yourself with, when in doubt use `CurrentThread` instead. If you're looking
/// to spawn a list of futures within a future you're likely going to want
/// `FuturesUnordered`. Using `TaskRunner` typically implies that you're
/// integraing the `futures` crate into a foreign event loop (aka not Tokio
/// which already knows about `futures`).
pub struct TaskRunner<W> {
    inner: Rc<Inner>,
    futures: Scheduler<SpawnedFuture, W>,
}

struct Inner {
    new_futures: RefCell<Vec<SpawnedFuture>>,
    canceled: Cell<bool>,
    non_daemons: Cell<usize>,
}

struct SpawnedFuture {
    inner: Spawn<Box<Future<Item = bool, Error = bool>>>,
}

impl<W: Wakeup> TaskRunner<W> {
    /// Creates a new `TaskRunner` with a provided `wakeup` handle.
    ///
    /// The task runner returned is ready to have futures spawned onto it and
    /// to have the `poll` method called. The `poll` method, when it returns,
    /// will be scheduled to send wakeup notifications to the `wakeup` handle
    /// provided here. In other words, the futures spawned onto this task
    /// runner will route their notifications of readiness to `wakeup` to
    /// ensure that the caller and user of `TaskRunner` knows when to call
    /// `poll` again.
    pub fn new(wakeup: W) -> TaskRunner<W> {
        TaskRunner {
            futures: Scheduler::new(wakeup),
            inner: Rc::new(Inner {
                canceled: Cell::new(false),
                new_futures: RefCell::new(Vec::new()),
                non_daemons: Cell::new(0),
            }),
        }
    }

    /// Returns a handle to this `TaskRunner` which is used to execute new
    /// futures.
    ///
    /// This method effectively returns a handle of sorts to this instance of
    /// `TaskRunner`. The handle can then be used with the `Executor` trait to
    /// spawn any futures that are `'static`. Spawned futures will be pushed
    /// towards completion during calls to `poll` below.
    pub fn executor(&self) -> TaskExecutor {
        TaskExecutor { inner: Rc::downgrade(&self.inner) }
    }

    /// Returns a boolean to indiate, if at this time, this task runner is
    /// finished executing tasks.
    ///
    /// A task runner finishes when either it's been canceled through the
    /// `TaskExecutor::cancel_all_executing` method or if all *non daemon*
    /// futures have finished (those spawned through `TaskExecutor::execute`).
    pub fn is_done(&self) -> bool {
        self.inner.canceled.get() ||
            (self.inner.non_daemons.get() == 0 &&
             self.inner.new_futures.borrow().is_empty())
    }

    /// Performs as much work as is necessary for the internal list of futures
    /// without blocking.
    ///
    /// This function is the primary method for driving the internal list of
    /// futures to completion. A call to `poll` will attempt to make as much
    /// progress as it can internally without blocking, polling any futures
    /// that may be ready and processing any requests tospawn futures.
    ///
    /// # Panics
    ///
    /// This method will panic if an executor context has already been entered.
    /// In other words this method will call `executor::enter` and panic if a
    /// parent stack frame of this thread has also called `enter`. For example
    /// if this function is called within `CurrentThread::run` or itself it
    /// will panic.
    ///
    /// A `TaskRunner` is not intended to be used for recursive execution of
    /// futures but rather as the sole and single point of spawning futures for
    /// a thread in an application.
    ///
    /// For more information about this source of panics see the documentation
    /// of `CurrentThread::run`.
    ///
    /// This method also does not attempt to catch panics of the underlying
    /// futures.  Instead it will propagate panics if any polled future panics.
    pub fn poll(&mut self) {
        set_current(&self.executor(), |_enter| self._poll())
    }

    fn _poll(&mut self) {
        loop {
            // Make progress on all spawned futures as long as we can
            while !self.is_done() {
                let res = self.futures.tick(|_scheduler, future, notify| {
                    future.inner.poll_future_notify(notify, 0)
                        .unwrap_or_else(Async::Ready)
                });
                match res {
                    Tick::Data(/* is_daemon = */ true) => {}
                    Tick::Data(false) => {
                        self.inner.non_daemons.set(self.inner.non_daemons.get() - 1);
                    }
                    Tick::Inconsistent |
                    Tick::Empty => break,
                }
            }

            // Now that we've made as much progress as we can, check our list of
            // spawned futures to see if anything was spawned
            let mut futures = self.inner.new_futures.borrow_mut();
            if futures.len() == 0 {
                break
            }
            for future in futures.drain(..) {
                self.futures.schedule(future);
            }
        }
    }
}

impl<W: Wakeup + fmt::Debug> fmt::Debug for TaskRunner<W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskRunner").finish()
    }
}

/// A handle to a `TaskRunner` to spawn new futures an interact with it.
///
/// This is created by the `TaskRunner::executor` method and primarily serves
/// the purpose of a trait implementation of `Exeuctor`. Note that you likely
/// need not worry yourself too much about this type, when in doubt consult the
/// documentation of `TaskRunner` itself.
pub struct TaskExecutor {
    inner: Weak<Inner>,
}

impl TaskExecutor {
    /// Routes requests to `CurrentThread` for spawning futures to this
    /// `TaskExecutor`.
    ///
    /// This method will install this instance of a `TaskExecutor` as the
    /// backing implementation behind the `CurrentThread` functions like
    /// `CurrentThread::execute` and `CurrentThread::execute_daemon`. Within
    /// the closure `f` any requests to these functions on `CurrentThread`
    /// will be routed to this instance of `TaskExecutor`.
    ///
    /// # Panics
    ///
    /// This function will transitively call `executor::enter` and panic if it
    /// fails. In other words this function cannot be used to recursively
    /// install a *different* implementation for `CurrentThread` than is
    /// already available. Instead this is intended to only be used as the
    /// *one* location for this thread where `CurrentThread` gets a backing
    /// implementation.
    ///
    /// For more information about this source of panics see the documentation
    /// of `CurrentThread::run`.
    pub fn with_current_thread_using_me<F, R>(&self, f: F) -> R
        where F: FnOnce() -> R
    {
        set_current(self, |_enter| f())
    }

    /// Flags to the bound `TaskRunner` instance that it should cease execution
    /// of all futures immediately.
    ///
    /// This function is used to prevent the `poll` function from ever doing
    /// any more work. After this function is called the `TaskRunner::is_done`
    /// method will return true and `poll` will always be a noop.
    pub fn cancel_all_executing(&self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.canceled.set(true);
        }
    }

    /// Executes a future, like the `Executor` trait, but as a *daemon*.
    ///
    /// A daemon future in this context is one that is defined as not
    /// influencing the return value of `TaskRunner::is_done`. Daemon futures
    /// progress as other futures do, however (on calls to `TaskRunner::poll`).
    pub fn execute_daemon<F>(&self, future: F) -> Result<(), ExecuteError<F>>
        where F: Future<Item = (), Error = ()> + 'static,
    {
        spawn(&self.inner, future, true)
    }
}

impl<F> Executor<F> for TaskExecutor
    where F: Future<Item = (), Error = ()> + 'static
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        spawn(&self.inner, future, false)
    }
}

impl fmt::Debug for TaskExecutor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskExecutor").finish()
    }
}

fn spawn<F>(inner: &Weak<Inner>, future: F, daemon: bool)
    -> Result<(), ExecuteError<F>>
    where F: Future<Item = (), Error = ()> + 'static,
{
    match inner.upgrade() {
        Some(inner) => {
            if !daemon {
                inner.non_daemons.set(inner.non_daemons.get() + 1);
            }
            _spawn(&inner, Box::new(future.then(move |_| Ok(daemon))));
            Ok(())
        }
        None => Err(ExecuteError::new(ExecuteErrorKind::Shutdown, future)),
    }
}

fn _spawn(inner: &Inner, future: Box<Future<Item = bool, Error = bool>>) {
    inner.new_futures.borrow_mut().push(SpawnedFuture {
        inner: executor::spawn(future),
    });
}

thread_local!(static CURRENT: Cell<*const TaskExecutor> = Cell::new(0 as *const _));

pub fn set_current<F, R>(current: &TaskExecutor, f: F) -> R
    where F: FnOnce(Enter) -> R
{
    let enter = executor::enter()
        .expect("cannot execute `CurrentThread` executor from within \
                 another executor");

    CURRENT.with(|c| {
        struct Reset<'a, T: 'a>(&'a Cell<*const T>);

        impl<'a, T> Drop for Reset<'a, T> {
            fn drop(&mut self) {
                self.0.set(ptr::null());
            }
        }

        assert!(c.get().is_null());
        let _reset = Reset(c);
        c.set(current);
        f(enter)
    })
}

pub fn with_current<F, R>(f: F) -> R
    where F: FnOnce(Option<&TaskExecutor>) -> R
{
    CURRENT.with(|c| {
        f(if c.get().is_null() {
            None
        } else {
            Some(unsafe { &*c.get() })
        })
    })
}
