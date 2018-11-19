use crate::stream::{Fuse, FuturesOrdered};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Waker, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::fmt;
use std::pin::Pin;

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they were pulled out of the original stream. This
/// is created by the `Stream::buffered` method.
#[must_use = "streams do nothing unless polled"]
pub struct Buffered<St: Stream>
where
    St: Stream,
    St::Item: Future,
{
    stream: Fuse<St>,
    in_progress_queue: FuturesOrdered<St::Item>,
    max: usize,
}

impl<St> Unpin for Buffered<St>
where
    St: Stream + Unpin,
    St::Item: Future,
{}

impl<St> fmt::Debug for Buffered<St>
where
    St: Stream + fmt::Debug,
    St::Item: Future,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Buffered")
            .field("stream", &self.stream)
            .field("in_progress_queue", &self.in_progress_queue)
            .field("max", &self.max)
            .finish()
    }
}

impl<St> Buffered<St>
where
    St: Stream,
    St::Item: Future,
{
    unsafe_pinned!(stream: Fuse<St>);
    unsafe_unpinned!(in_progress_queue: FuturesOrdered<St::Item>);

    pub(super) fn new(stream: St, n: usize) -> Buffered<St> {
        Buffered {
            stream: super::Fuse::new(stream),
            in_progress_queue: FuturesOrdered::new(),
            max: n,
        }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        self.stream.get_mut()
    }

    /// Acquires a mutable pinned reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    #[allow(clippy::needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        unsafe { Pin::map_unchecked_mut(self, |x| x.get_mut()) }
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }
}

impl<St> Stream for Buffered<St>
where
    St: Stream,
    St::Item: Future,
{
    type Item = <St::Item as Future>::Output;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Option<Self::Item>> {
        // Try to spawn off as many futures as possible by filling up
        // our in_progress_queue of futures.
        while self.in_progress_queue.len() < self.max {
            match self.as_mut().stream().poll_next(waker) {
                Poll::Ready(Some(fut)) => self.as_mut().in_progress_queue().push(fut),
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Attempt to pull the next value from the in_progress_queue
        let res = Pin::new(self.as_mut().in_progress_queue()).poll_next(waker);
        if let Some(val) = ready!(res) {
            return Poll::Ready(Some(val))
        }

        // If more values are still coming from the stream, we're not done yet
        if self.stream.is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Buffered<S>
where
    S: Stream + Sink,
    S::Item: Future,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
