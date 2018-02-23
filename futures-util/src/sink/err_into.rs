use futures_core::{Stream, Poll};
use futures_core::task;
use futures_sink::{Sink};
use sink::{SinkExt, SinkMapErr};

/// A sink combinator to change the error type of a sink.
///
/// This is created by the `Sink::err_into` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct SinkErrInto<S: Sink, E> {
    sink: SinkMapErr<S, fn(S::SinkError) -> E>,
}

pub fn new<S, E>(sink: S) -> SinkErrInto<S, E>
    where S: Sink,
          S::SinkError: Into<E>
{
    SinkErrInto {
        sink: SinkExt::sink_map_err(sink, Into::into),
    }
}

impl<S: Sink, E> SinkErrInto<S, E> {
    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        self.sink.get_ref()
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut S {
        self.sink.get_mut()
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.sink.into_inner()
    }
}

impl<S, E> Sink for SinkErrInto<S, E>
    where S: Sink,
          S::SinkError: Into<E>,
{
    type SinkItem = S::SinkItem;
    type SinkError = E;

    delegate_sink!(sink);
}

impl<S: Sink + Stream, E> Stream for SinkErrInto<S, E> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll_next(cx)
    }
}
