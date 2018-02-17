use futures_core::{Poll, Async, Future};
use futures_core::task;
use futures_sink::{Sink};

/// Future for the `Sink::send` combinator, which sends a value to a sink and
/// then waits until the sink has fully flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Send<S: Sink> {
    sink: Option<S>,
    item: Option<S::SinkItem>,
}

pub fn new<S: Sink>(sink: S, item: S::SinkItem) -> Send<S> {
    Send {
        sink: Some(sink),
        item: Some(item),
    }
}

impl<S: Sink> Send<S> {
    /// Get a shared reference to the inner sink.
    ///
    /// Returns `None` if the future has completed already.
    pub fn get_ref(&self) -> Option<&S> {
        self.sink.as_ref()
    }

    /// Get a mutable reference to the inner sink.
    ///
    /// Returns `None` if the future has completed already.
    pub fn get_mut(&mut self) -> Option<&mut S> {
        self.sink.as_mut()
    }

    fn sink_mut(&mut self) -> &mut S {
        self.sink.as_mut().take().expect("Attempted to poll Send after completion")
    }

    fn take_sink(&mut self) -> S {
        self.sink.take().expect("Attempted to poll Send after completion")
    }
}

impl<S: Sink> Future for Send<S> {
    type Item = S;
    type Error = S::SinkError;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<S, S::SinkError> {
        if let Some(item) = self.item.take() {
            match self.sink_mut().poll_ready(cx)? {
                Async::Ready(()) => {
                    self.sink_mut().start_send(item)?;
                }
                Async::Pending => {
                    self.item = Some(item);
                    return Ok(Async::Pending);
                }
            }
        }

        // we're done sending the item, but want to block on flushing the
        // sink
        try_ready!(self.sink_mut().poll_flush(cx));

        // now everything's emptied, so return the sink for further use
        Ok(Async::Ready(self.take_sink()))
    }
}
