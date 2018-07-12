use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// A stream combinator which returns a maximum number of elements.
///
/// This structure is produced by the `Stream::take` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Take<St> {
    stream: St,
    remaining: u64,
}

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
        mut self: PinMut<Self>,
        cx: &mut task::Context
    ) -> Poll<Option<St::Item>> {
        if *self.remaining() == 0 {
            Poll::Ready(None)
        } else {
            let next = ready!(self.stream().poll_next(cx));
            match next {
                Some(_) => *self.remaining() -= 1,
                None => *self.remaining() = 0,
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
