use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Stream for the [`inspect`](super::StreamExt::inspect) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Inspect<St, F> where St: Stream {
    stream: St,
    f: F,
}

impl<St: Stream + Unpin, F> Unpin for Inspect<St, F> {}

impl<St, F> Inspect<St, F>
    where St: Stream,
          F: FnMut(&St::Item),
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);

    pub(super) fn new(stream: St, f: F) -> Inspect<St, F> {
        Inspect { stream, f }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: Stream + FusedStream, F> FusedStream for Inspect<St, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F> Stream for Inspect<St, F>
    where St: Stream,
          F: FnMut(&St::Item),
{
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St::Item>> {
        let item = ready!(self.as_mut().stream().poll_next(cx));
        Poll::Ready(item.map(|e| {
            (self.as_mut().f())(&e);
            e
        }))
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, F, Item> Sink<Item> for Inspect<S, F>
    where S: Stream + Sink<Item>,
          F: FnMut(&S::Item),
{
    type SinkError = S::SinkError;

    delegate_sink!(stream, Item);
}
