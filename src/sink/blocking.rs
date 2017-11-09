use {Async, AsyncSink};
use sink::Sink;
use executor;
use task_impl::ThreadNotify;

/// Provides thread-blocking operations on a sink.
///
/// See [`blocking`](fn.blocking.html) documentation for more details.
#[derive(Debug)]
#[must_use = "sinks do nothing unless used"]
pub struct Blocking<T: Sink> {
    inner: Option<executor::Spawn<T>>,
}

/// Provides thread-blocking operations on a sink.
///
/// `blocking` consumes ownership of `sink`, returning a `Blocking` backed by
/// the sink. The `Blocking` value exposes thread-blocking operations that allow
/// sending values into the sink. For example, `Blocking::send` will block the
/// current thread until the inner sink has capacity to accept the item being
/// sent.
///
/// **These operations will block the current thread**. This means that they
/// should not be called while in the context of a task executor as this will
/// block the task executor's progress. Also, note that **this value will block
/// the current thread on drop**. Droping `Blocking` will ensure that the inner
/// sink is fully flushed before completing the drop. This could block the
/// current thread.
pub fn blocking<T: Sink>(sink: T) -> Blocking<T> {
    let inner = executor::spawn(sink);
    Blocking { inner: Some(inner) }
}

impl<T: Sink> Blocking<T> {
    /// Sends a value to this sink, blocking the current thread until it's able
    /// to do so.
    ///
    /// This function will take the `value` provided and call the underlying
    /// sink's `start_send` function until it's ready to accept the value. If
    /// the function returns `NotReady` then the current thread is blocked
    /// until it is otherwise ready to accept the value.
    ///
    /// # Return value
    ///
    /// If `Ok(())` is returned then the `value` provided was successfully sent
    /// along the sink, and if `Err(e)` is returned then an error occurred
    /// which prevented the value from being sent.
    pub fn send(&mut self, mut value: T::SinkItem) -> Result<(), T::SinkError> {
        ThreadNotify::with_current(|notify| {
            loop {
                let inner = self.inner.as_mut().unwrap();
                match inner.start_send_notify(value, notify, 0) {
                    Ok(AsyncSink::Ready) => return Ok(()),
                    Ok(AsyncSink::NotReady(v)) => {
                        value = v;
                        notify.park();
                    }
                    Err(e) => return Err(e),
                }
            }
        })
    }

    /// Flushes any buffered data in this sink, blocking the current thread
    /// until it's entirely flushed.
    ///
    /// This function will call the underlying sink's `poll_complete` method
    /// until it returns that it's ready to proceed. If the method returns
    /// `NotReady` the current thread will be blocked until it's otherwise
    /// ready to proceed.
    pub fn flush(&mut self) -> Result<(), T::SinkError> {
        ThreadNotify::with_current(|notify| {
            loop {
                let inner = self.inner.as_mut().unwrap();
                match inner.poll_flush_notify(notify, 0) {
                    Ok(Async::Ready(_)) => return Ok(()),
                    Ok(Async::NotReady) => notify.park(),
                    Err(e) => return Err(e),
                }
            }
        })
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &T {
        self.inner.as_ref().unwrap().get_ref()
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap().get_mut()
    }

    /// Consume the `Blocking`, returning its inner sink.
    pub fn into_inner(mut self) -> T {
        self.inner.take().unwrap().into_inner()
    }
}

impl<T: Sink> Drop for Blocking<T> {
    fn drop(&mut self) {
        ThreadNotify::with_current(|notify| {
            if let Some(ref mut inner) = self.inner {
                loop {
                    match inner.close_notify(notify, 0) {
                        Ok(Async::Ready(_)) => break,
                        Ok(Async::NotReady) => notify.park(),
                        Err(_) => break,
                    }
                }
            }
        })
    }
}
