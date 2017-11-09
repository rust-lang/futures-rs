#![allow(deprecated)]

use sink::Sink;
use executor;

#[must_use = "sinks do nothing unless used"]
#[doc(hidden)]
#[deprecated(note = "use `sink::blocking` instead")]
#[derive(Debug)]
pub struct Wait<S> {
    sink: executor::Spawn<S>,
}

pub fn new<S: Sink>(s: S) -> Wait<S> {
    Wait {
        sink: executor::spawn(s),
    }
}

impl<S: Sink> Wait<S> {
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
    #[allow(deprecated)]
    pub fn send(&mut self, value: S::SinkItem) -> Result<(), S::SinkError> {
        self.sink.wait_send(value)
    }

    /// Flushes any buffered data in this sink, blocking the current thread
    /// until it's entirely flushed.
    ///
    /// This function will call the underlying sink's `poll_complete` method
    /// until it returns that it's ready to proceed. If the method returns
    /// `NotReady` the current thread will be blocked until it's otherwise
    /// ready to proceed.
    #[allow(deprecated)]
    pub fn flush(&mut self) -> Result<(), S::SinkError> {
        self.sink.wait_flush()
    }

    /// Close this sink, blocking the current thread until it's entirely closed.
    ///
    /// This function will call the underlying sink's `close` method
    /// until it returns that it's closed. If the method returns
    /// `NotReady` the current thread will be blocked until it's otherwise closed.
    #[allow(deprecated)]
    pub fn close(&mut self) -> Result<(), S::SinkError> {
        self.sink.wait_close()
    }
}
