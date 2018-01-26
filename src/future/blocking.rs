use Async;
use future::Future;
use executor::{self, NotifyHandle};
use task_impl::ThreadNotify;

/// Provides thread-blocking operations on a future.
///
/// See [`blocking`](fn.blocking.html) documentation for more details.
#[derive(Debug)]
#[must_use = "futures do nothing unless used"]
pub struct Blocking<T> {
    inner: executor::Spawn<T>,
}

/// Provides thread-blocking operations on a future.
///
/// `blocking` consumes ownership of `future`, returning a `Blocking` backed by
/// the future. The `Blocking` value exposes thread-blocking operations that
/// allow getting the realized value of the future. For example,
/// `Blocking::wait` will block the current thread until the inner future has
/// completed and return the completed value.
///
/// **These operations will block the current thread**. This means that they
/// should not be called while in the context of a task executor as this will
/// block the task executor's progress.
pub fn blocking<T: Future>(future: T) -> Blocking<T> {
    let inner = executor::spawn(future);
    Blocking { inner: inner }
}

impl<T: Future> Blocking<T> {
    /// Query the inner future to see if its value has become available.
    ///
    /// Unlike `Future::poll`, this function does **not** register interest if
    /// the inner future is not in a ready state.
    ///
    /// This function will return immediately if the inner future is not ready.
    pub fn try_take(&mut self) -> Option<Result<T::Item, T::Error>> {
        match self.inner.poll_future_notify(&NotifyHandle::noop(), 0) {
            Ok(Async::NotReady) => None,
            Ok(Async::Ready(v)) => Some(Ok(v)),
            Err(e) => Some(Err(e)),
        }
    }

    /// Block the current thread until this future is resolved.
    ///
    /// This method will drive the inner future to completion via
    /// `Future::poll`. **The current thread will be blocked** until the future
    /// transitions to a ready state. Once the future is complete, the result of
    /// this future is returned.
    ///
    /// > **Note:** This method is not appropriate to call on event loops or
    /// >           similar I/O situations because it will prevent the event
    /// >           loop from making progress (this blocks the thread). This
    /// >           method should only be called when it's guaranteed that the
    /// >           blocking work associated with this future will be completed
    /// >           by another thread.
    ///
    /// This method is only available when the `use_std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Panics
    ///
    /// This method panics if called from within an executor.
    ///
    /// This method does not attempt to catch panics. If the `poll` function of
    /// the inner future panics, the panic will be propagated to the caller.
    pub fn wait(&mut self) -> Result<T::Item, T::Error> {
        let _enter = executor::enter()
            .expect("cannot call `future::Blocking::wait` from within \
                     another executor.");

        ThreadNotify::with_current(|notify| {
            loop {
                match self.inner.poll_future_notify(notify, 0)? {
                    Async::NotReady => notify.park(),
                    Async::Ready(e) => return Ok(e),
                }
            }
        })
    }
}

impl<T> Blocking<T> {
    /// Get a shared reference to the inner future.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the inner future.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Consume the `Blocking`, returning its inner future.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}
