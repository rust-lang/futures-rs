use crate::sink::{SinkExt, SinkMapErr};
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::{Sink};
use pin_project::{pin_project, unsafe_project};

/// Sink for the [`sink_err_into`](super::SinkExt::sink_err_into) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct SinkErrInto<Si: Sink<Item>, Item, E> {
    #[pin]
    sink: SinkMapErr<Si, fn(Si::Error) -> E>,
}

impl<Si, E, Item> SinkErrInto<Si, Item, E>
    where Si: Sink<Item>,
          Si::Error: Into<E>,
{
    pub(super) fn new(sink: Si) -> Self {
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

    /// Get a pinned mutable reference to the inner sink.
    #[pin_project(self)]
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Si> {
        self.sink.get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> Si {
        self.sink.into_inner()
    }
}

impl<Si, Item, E> Sink<Item> for SinkErrInto<Si, Item, E>
    where Si: Sink<Item>,
          Si::Error: Into<E>,
{
    type Error = E;

    delegate_sink!(sink, Item);
}

impl<S, Item, E> Stream for SinkErrInto<S, Item, E>
    where S: Sink<Item> + Stream,
          S::Error: Into<E>
{
    type Item = S::Item;

    #[pin_project(self)]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<S::Item>> {
        self.sink.poll_next(cx)
    }
}
