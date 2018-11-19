use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which will change the type of a stream from one
/// type to another.
///
/// This is produced by the `Stream::map` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Map<St, F> {
    stream: St,
    f: F,
}

impl<St: Unpin, F> Unpin for Map<St, F> {}

impl<St, T, F> Map<St, F>
    where St: Stream,
          F: FnMut(St::Item) -> T,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);

    pub(super) fn new(stream: St, f: F) -> Map<St, F> {
        Map { stream, f }
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

impl<St: FusedStream, F> FusedStream for Map<St, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F, T> Stream for Map<St, F>
    where St: Stream,
          F: FnMut(St::Item) -> T,
{
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Option<T>> {
        let option = ready!(self.as_mut().stream().poll_next(waker));
        Poll::Ready(option.map(self.as_mut().f()))
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, F> Sink for Map<S, F>
    where S: Sink
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/
