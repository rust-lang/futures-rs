use {Poll, Async, Future};
use Sink;

/// Future for the `Sink::flush` combinator, which polls the sink until all data
/// has been flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Flush<S> {
    sink: Option<S>,
}

pub fn new<S: Sink>(sink: S) -> Flush<S> {
    Flush { sink: Some(sink) }
}

impl<S: Sink> Flush<S> {
    /// Get a shared reference to the inner sink.
    /// Returns `None` if the sink has already been flushed.
    pub fn get_ref(&self) -> Option<&S> {
        self.sink.as_ref()
    }

    /// Get a mutable reference to the inner sink.
    /// Returns `None` if the sink has already been flushed.
    pub fn get_mut(&mut self) -> Option<&mut S> {
        self.sink.as_mut()
    }

    /// Consume the `Flush` and return the inner sink.
    /// Returns `None` if the sink has already been flushed.
    pub fn into_inner(self) -> Option<S> {
        self.sink
    }
}

impl<S: Sink> Future for Flush<S> {
    type Item = S;
    type Error = S::SinkError;

    fn poll(&mut self) -> Poll<S, S::SinkError> {
        let mut sink = self.sink.take().expect("Attempted to poll Flush after it completed");
        if sink.poll_complete()?.is_ready() {
            Ok(Async::Ready(sink))
        } else {
            self.sink = Some(sink);
            Ok(Async::Pending)
        }
    }
}
