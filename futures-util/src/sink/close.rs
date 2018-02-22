use futures_core::{Poll, Async, Future};
use futures_core::task;
use futures_sink::Sink;

/// Future for the `close` combinator, which polls the sink until all data has
/// been closeed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Close<S> {
    sink: Option<S>,
}

/// A future that completes when the sink has finished closing.
///
/// The sink itself is returned after closeing is complete.
pub fn close<S: Sink>(sink: S) -> Close<S> {
    Close { sink: Some(sink) }
}

impl<S: Sink> Close<S> {
    /// Get a shared reference to the inner sink.
    /// Returns `None` if the sink has already been closeed.
    pub fn get_ref(&self) -> Option<&S> {
        self.sink.as_ref()
    }

    /// Get a mutable reference to the inner sink.
    /// Returns `None` if the sink has already been closeed.
    pub fn get_mut(&mut self) -> Option<&mut S> {
        self.sink.as_mut()
    }

    /// Consume the `Close` and return the inner sink.
    /// Returns `None` if the sink has already been closeed.
    pub fn into_inner(self) -> Option<S> {
        self.sink
    }
}

impl<S: Sink> Future for Close<S> {
    type Item = S;
    type Error = S::SinkError;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<S, S::SinkError> {
        let mut sink = self.sink.take().expect("Attempted to poll Close after it completed");
        if sink.poll_close(cx)?.is_ready() {
            Ok(Async::Ready(sink))
        } else {
            self.sink = Some(sink);
            Ok(Async::Pending)
        }
    }
}
