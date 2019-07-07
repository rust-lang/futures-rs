use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};

/// Stream for the [`skip`](super::StreamExt::skip) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Skip<St> {
    #[pin]
    stream: St,
    remaining: u64,
}

impl<St: Stream> Skip<St> {
    pub(super) fn new(stream: St, n: u64) -> Skip<St> {
        Skip {
            stream,
            remaining: n,
        }
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
    #[pin_project(self)]
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: FusedStream> FusedStream for Skip<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: Stream> Stream for Skip<St> {
    type Item = St::Item;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St::Item>> {
        while *self.remaining > 0 {
            match ready!(self.stream.as_mut().poll_next(cx)) {
                Some(_) => *self.remaining -= 1,
                None => return Poll::Ready(None),
            }
        }

        self.stream.poll_next(cx)
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Skip<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
