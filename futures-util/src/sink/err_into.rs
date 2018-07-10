use crate::sink::{SinkExt, SinkMapErr};
use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use futures_sink::{Sink};

/// A sink combinator to change the error type of a sink.
///
/// This is created by the `Sink::err_into` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct SinkErrInto<Si: Sink, E> {
    sink: SinkMapErr<Si, fn(Si::SinkError) -> E>,
}

impl<Si, E> SinkErrInto<Si, E>
    where Si: Sink,
          Si::SinkError: Into<E>,
{
    unsafe_pinned!(sink: SinkMapErr<Si, fn(Si::SinkError) -> E>);

    pub(super) fn new(sink: Si) -> SinkErrInto<Si, E> {
        SinkErrInto {
            sink: SinkExt::sink_map_err(sink, Into::into),
        }
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &Si {
        self.sink.get_ref()
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut Si {
        self.sink.get_mut()
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> Si {
        self.sink.into_inner()
    }
}

impl<Si, E> Sink for SinkErrInto<Si, E>
    where Si: Sink,
          Si::SinkError: Into<E>,
{
    type SinkItem = Si::SinkItem;
    type SinkError = E;

    delegate_sink!(sink);
}

impl<S, E> Stream for SinkErrInto<S, E>
    where S: Sink + Stream,
          S::SinkError: Into<E>
{
    type Item = S::Item;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}
