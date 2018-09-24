use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which skips a number of elements before continuing.
///
/// This structure is produced by the `Stream::skip` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Skip<St> {
    stream: St,
    remaining: u64,
}

impl<St: Unpin> Unpin for Skip<St> {}

impl<St: Stream> Skip<St> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(remaining: u64);

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

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: Stream> Stream for Skip<St> {
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<St::Item>> {
        while *self.remaining() > 0 {
            match ready!(self.stream().poll_next(cx)) {
                Some(_) => *self.remaining() -= 1,
                None => return Poll::Ready(None),
            }
        }

        self.stream().poll_next(cx)
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Skip<S>
    where S: Sink
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/
