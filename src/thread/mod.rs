
#![allow(warnings, missing_docs)]

use std::prelude::v1::*;

use std::cell::{RefCell, Cell};
use std::fmt;
use std::ptr;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use {Async, Poll, Stream};
use executor::{self, Spawn, NotifyHandle, Notify, enter, Enter};
use future::{self, Future, Executor, ExecuteError, ExecuteErrorKind};
use stream::FuturesUnordered;
use task_impl::ThreadNotify;

mod sink;
mod stream;

pub use self::sink::BlockingSink;
pub use self::stream::BlockingStream;

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
    TaskRunner::enter(|_tr| future)
}

/// Execute the given closure, then block until all spawned tasks complete.
///
/// In more detail, this function will block until:
/// - All spawned tasks are complete, or
/// - `cancel_all_spawned` is invoked.
pub fn block_with_init<F>(f: F) where F: FnOnce(&Enter) {
    drop(TaskRunner::enter(|tr| {
        f(&tr.enter);
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
pub fn spawn_task<F>(task: F) where F: Future<Item = (), Error = ()> + 'static {
    TaskRunner::with(|t| t.spawn(Box::new(task), false))
        .unwrap_or_else(|()| {
            panic!("cannot call `spawn_task` unless your thread is already \
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
pub fn spawn_daemon<F>(task: F) where F: Future<Item = (), Error = ()> + 'static {
    TaskRunner::with(|t| t.spawn(Box::new(task), true))
        .unwrap_or_else(|()| {
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
    TaskRunner::with(|t| t.cancel_all_spawned())
        .unwrap_or_else(|()| {
            panic!("cannot call `cancel_all_spawned` unless your thread is \
                    already in the context of a call to `block_on_all` or \
                    `block_with_init`")
        })
}

/// Provides an executor handle for spawning tasks onto the current thread.
///
/// # Panics
///
/// As with the `spawn_*` functions, this function can only be invoked within
/// a future given to a `block_*` invocation; any other use will result in
/// a panic.
pub fn task_executor() -> TaskExecutor {
    TaskRunner::with(|t| TaskExecutor { inner: Rc::downgrade(t) })
        .unwrap_or_else(|()| {
            panic!("cannot call `task_executor` unless your thread is \
                    already in the context of a call to `block_on_all` or \
                    `block_with_init`")
        })
}

pub struct TaskExecutor {
    inner: Weak<Inner>,
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

/// Provides an executor handle for spawning daemons onto the current thread.
///
/// # Panics
///
/// As with the `spawn_*` functions, this function can only be invoked within
/// a future given to a `block_*` invocation; any other use will result in
/// a panic.
pub fn daemon_executor() -> DaemonExecutor {
    TaskRunner::with(|t| DaemonExecutor { inner: Rc::downgrade(t) })
        .unwrap_or_else(|()| {
            panic!("cannot call `daemon_executor` unless your thread is \
                    already in the context of a call to `block_on_all` or \
                    `block_with_init`")
        })
}

pub struct DaemonExecutor {
    inner: Weak<Inner>,
}

impl<F> Executor<F> for DaemonExecutor
    where F: Future<Item = (), Error = ()> + 'static
{
    fn execute(&self, future: F) -> Result<(), ExecuteError<F>> {
        spawn(&self.inner, future, true)
    }
}

impl fmt::Debug for DaemonExecutor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DaemonExecutor").finish()
    }
}

fn spawn<F>(inner: &Weak<Inner>, future: F, daemon: bool)
    -> Result<(), ExecuteError<F>>
    where F: Future<Item = (), Error = ()> + 'static,
{
    let inner = match inner.upgrade() {
        Some(inner) => inner,
        None => return Err(ExecuteError::new(ExecuteErrorKind::Shutdown, future)),
    };
    inner.spawn(Box::new(future), daemon);
    Ok(())
}

// An object for cooperatively executing multiple tasks on a single thread.
// Useful for working with non-`Send` futures.
//
// NB: this is not `Send`
struct TaskRunner {
    inner: Rc<Inner>,
    non_daemons: usize,
    futures: Spawn<FuturesUnordered<SpawnedFuture>>,
    enter: Enter,
}

struct Inner {
    new_futures: RefCell<Vec<(Box<Future<Item=(), Error=()>>, bool)>>,
    cancel: Cell<bool>,
}

thread_local!(static MY_TASK_RUNNER: RefCell<Option<Rc<Inner>>> = RefCell::new(None));

impl TaskRunner {
    fn new() -> TaskRunner {
        TaskRunner {
            non_daemons: 0,
            futures: executor::spawn(FuturesUnordered::new()),
            inner: Rc::new(Inner {
                new_futures: RefCell::new(Vec::new()),
                cancel: Cell::new(false),
            }),
            enter: enter(),
        }
    }

    fn enter<F, A>(f: F) -> Result<A::Item, A::Error>
        where F: FnOnce(&TaskRunner) -> A,
              A: Future,
    {
        let mut tr = TaskRunner::new();
        let future = f(&tr);
        MY_TASK_RUNNER.with(|t| {
            assert!(t.borrow().is_none());
            *t.borrow_mut() = Some(tr.inner.clone());

            struct Reset<'a>(&'a RefCell<Option<Rc<Inner>>>);
            impl<'a> Drop for Reset<'a> {
                fn drop(&mut self) {
                    *self.0.borrow_mut() = None;
                }
            }
            let _reset = Reset(t);

            ThreadNotify::with_current(|notify| {
                tr.finish(notify, future)
            })
        })
    }

    fn with<F, R>(f: F) -> Result<R, ()>
        where F: FnOnce(&Rc<Inner>) -> R,
    {
        MY_TASK_RUNNER.with(|t| {
            let inner = {
                let slot = t.borrow();
                match *slot {
                    Some(ref inner) => inner.clone(),
                    None => return Err(()),
                }
            };
            Ok(f(&inner))
        })
    }

    fn finish<F: Future>(&mut self, notify: &Arc<ThreadNotify>, future: F)
        -> Result<F::Item, F::Error>
    {
        let mut result = None;
        let mut future = Some(executor::spawn(future));
        while future.is_some() || self.non_daemons > 0 {
            if self.inner.cancel.get() {
                self.inner.cancel.set(false);
                self.futures = executor::spawn(FuturesUnordered::new());
            }
            let res = future.as_mut().map(|f| f.poll_future_notify(notify, 0));
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
            self.poll_all(notify);
            if future.is_some() || self.non_daemons > 0 {
                notify.park();
            }
        }
        result.unwrap()
    }

    fn poll_all(&mut self, notify: &Arc<ThreadNotify>) {
        loop {
            // Make progress on all spawned futures as long as we can
            loop {
                match self.futures.poll_stream_notify(notify, 0) {
                    // If a daemon exits, then we ignore it, but if a non-daemon
                    // exits then we update our counter of non-daemons
                    Ok(Async::Ready(Some(daemon))) |
                    Err(daemon) => {
                        if !daemon {
                            self.non_daemons -= 1;
                        }
                    }
                    Ok(Async::NotReady) |
                    Ok(Async::Ready(None)) => break,
                }
            }

            // Now that we've made as much progress as we can, check our list of
            // spawned futures to see if anything was spawned
            let mut futures = self.inner.new_futures.borrow_mut();
            if futures.len() == 0 {
                break
            }
            for (future, daemon) in futures.drain(..) {
                if !daemon {
                    self.non_daemons += 1;
                }
                self.futures.get_mut().push(SpawnedFuture {
                    daemon: daemon,
                    inner: future,
                });
            }
        }
    }
}

struct SpawnedFuture {
    daemon: bool,
    // TODO: wrap in `Spawn`
    inner: Box<Future<Item = (), Error = ()>>,
}

impl Future for SpawnedFuture {
    type Item = bool;
    type Error = bool;

    fn poll(&mut self) -> Poll<bool, bool> {
        match self.inner.poll() {
            Ok(e) => Ok(e.map(|()| self.daemon)),
            Err(()) => Err(self.daemon),
        }
    }
}

impl Inner {
    fn spawn(&self,
             future: Box<Future<Item = (), Error = ()>>,
             daemon: bool) {
        self.new_futures.borrow_mut().push((future, daemon));
    }

    fn cancel_all_spawned(&self) {
        self.cancel.set(true);
    }
}
