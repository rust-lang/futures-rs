use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which returns a maximum number of elements.
///
/// This structure is produced by the `Stream::take` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Take<St> {
    stream: St,
    remaining: u64,
}

impl<St: Unpin> Unpin for Take<St> {}

impl<St: Stream> Take<St> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(remaining: u64);

    pub(super) fn new(stream: St, n: u64) -> Take<St> {
        Take {
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

impl<St> Stream for Take<St>
    where St: Stream,
{
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker
    ) -> Poll<Option<St::Item>> {
        if self.remaining == 0 {
            Poll::Ready(None)
        } else {
            let next = ready!(self.as_mut().stream().poll_next(lw));
            match next {
                Some(_) => *self.as_mut().remaining() -= 1,
                None => *self.as_mut().remaining() = 0,
            }
            Poll::Ready(next)
        }
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Take<S>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/
