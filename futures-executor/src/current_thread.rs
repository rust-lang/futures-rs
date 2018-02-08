use futures_core::task::{self, NotifyHandle};
use futures_core::{Future, Async};

use thread::ThreadNotify;
use task_runner::{set_current, with_current, TaskRunner};

/// Provides execution context
///
/// This currently does not do anything, but allows future improvements to be
/// made in a backwards compatible way.
pub struct Context<'a> {
    // enter: &'a executor::Enter,
    runner: &'a mut TaskRunner,
    handle: &'a (Fn() -> NotifyHandle + 'a),
    thread: &'a ThreadNotify,
}

pub fn run<F, R>(f: F) -> R
    where F: FnOnce(&mut Context) -> R
{
    ThreadNotify::with_current(|thread| {
        let mut runner = TaskRunner::new();
        let handle = &|| thread.clone().into();

        // Kick off any initial work through the callback provided
        let ret = set_current(&runner.executor(), || {
            f(&mut Context {
                // enter: &,
                runner: &mut runner,
                handle,
                thread,
            })
        });

        // So long as there's pending work we keep polling and sleeping.
        if !runner.is_done() {
            loop {
                runner.poll(handle);
                if runner.is_done() {
                    break
                }
                thread.park();
            }
        }

        return ret
    })
}

/// Spawns a future on the current thread.
///
/// The provided future must complete or be canceled before
/// `run` will return.
///
/// # Panics
///
/// This function can only be invoked from the context of a
/// `run` call; any other use will result in a panic.
pub fn spawn<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
{
    _spawn(future, false).unwrap_or_else(|_| {
        panic!("cannot call `execute` unless the thread is already \
                in the context of a call to `run`")
    })
}

/// Spawns a daemon future on the current thread.
///
/// Completion of the provided future is not required for the pending
/// `run` call to complete. If `run` returns before `future` completes, it
/// will be dropped.
///
/// # Panics
///
/// This function can only be invoked from the context of a
/// `run` call; any other use will result in a panic.
pub fn spawn_daemon<F>(future: F)
    where F: Future<Item = (), Error = ()> + 'static
{
    _spawn(future, true).unwrap_or_else(|_| {
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
pub fn cancel_all_spawned() {
    with_current(|current| {
        current.expect("cannot call `cancel_all_executing` unless the \
                        thread is already in the context of a call to `run`")
            .cancel_all_executing()
    })
}

impl<'a> Context<'a> {
    // /// Returns a reference to the executor `Enter` handle.
    // pub fn enter(&self) -> &executor::Enter {
    //     &self.enter
    // }

    /// Synchronously waits for the provided `future` to complete.
    ///
    /// This function can be used to synchronously block the current thread
    /// until the provided `future` has resolved either successfully or with an
    /// error. The result of the future is then returned from this function
    /// call.
    ///
    /// Note that this function will *also* execute any spawned futures on the
    /// current thread. This function is only available on `Context` which can
    /// only be acquired from the `run` function in this module, which means
    /// that other futures may be spawned on the current thread as a daemon or
    /// not.
    pub fn block_on<F>(&mut self, future: F) -> Result<F::Item, F::Error>
        where F: Future,
    {
        let mut future = task::spawn(future);
        loop {
            match future.poll_future_notify(&::IntoNotifyHandle(self.handle), 0) {
                Ok(Async::Ready(e)) => return Ok(e),
                Err(e) => return Err(e),
                Ok(Async::Pending) => {}
            }
            self.runner.poll(self.handle);
            self.thread.park();
        }
    }
}

/// Submits a future to the current `CurrentThread` executor. This is done by
/// checking the thread-local variable tracking the current executor.
///
/// If this function is not called in context of an executor, i.e. outside of
/// `run`, then `Err` is returned.
///
/// This function does not panic.
fn _spawn<F>(future: F, daemon: bool) -> Result<(), ()>
    where F: Future<Item = (), Error = ()> + 'static,
{
    with_current(|current| {
        match current {
            Some(c) if daemon => Ok(c.spawn_daemon(future)),
            Some(c) => Ok(c.spawn(future)),
            None => Err(()),
        }
    })
}
