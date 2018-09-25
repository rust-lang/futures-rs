use crate::stream::{StreamExt, Fuse};
use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A `Stream` that implements a `peek` method.
///
/// The `peek` method can be used to retrieve a reference
/// to the next `Stream::Item` if available. A subsequent
/// call to `poll` will return the owned item.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Peekable<St: Stream> {
    stream: Fuse<St>,
    peeked: Option<St::Item>,
}

impl<St: Stream + Unpin> Unpin for Peekable<St> {}

impl<St: Stream> Peekable<St> {
    unsafe_pinned!(stream: Fuse<St>);
    unsafe_unpinned!(peeked: Option<St::Item>);

    pub(super) fn new(stream: St) -> Peekable<St> {
        Peekable {
            stream: stream.fuse(),
            peeked: None
        }
    }

    /// Peek retrieves a reference to the next item in the stream.
    ///
    /// This method polls the underlying stream and return either a reference
    /// to the next item if the stream is ready or passes through any errors.
    pub fn peek<'a>(
        self: &'a mut Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<&'a St::Item>> {
        if self.peeked().is_some() {
            return Poll::Ready(self.peeked().as_ref())
        }
        match ready!(self.stream().poll_next(lw)) {
            None => Poll::Ready(None),
            Some(item) => {
                *self.peeked() = Some(item);
                Poll::Ready(self.peeked().as_ref())
            }
        }
    }
}

impl<S: Stream> Stream for Peekable<S> {
    type Item = S::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker
    ) -> Poll<Option<Self::Item>> {
        if let Some(item) = self.peeked().take() {
            return Poll::Ready(Some(item))
        }
        self.stream().poll_next(lw)
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Peekable<S>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/
