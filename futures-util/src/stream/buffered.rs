use crate::stream::{Fuse, FuturesOrdered};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use futures_sink::Sink;
use std::fmt;
use std::marker::Unpin;
use std::mem::PinMut;

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they were pulled out of the original stream. This
/// is created by the `Stream::buffered` method.
#[must_use = "streams do nothing unless polled"]
pub struct Buffered<S: Stream>
where
    S: Stream,
    S::Item: Future,
{
    stream: Fuse<S>,
    in_progress_queue: FuturesOrdered<S::Item>,
    max: usize,
}

impl<S> Unpin for Buffered<S>
where
    S: Stream,
    S::Item: Future,
{}

impl<S: Stream> Buffered<S>
where
    S: Stream,
    S::Item: Future,
{
    unsafe_pinned!(stream -> Fuse<S>);
    unsafe_unpinned!(in_progress_queue -> FuturesOrdered<S::Item>);
}

impl<S> fmt::Debug for Buffered<S>
where
    S: Stream + fmt::Debug,
    S::Item: Future,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Buffered")
            .field("stream", &self.stream)
            .field("in_progress_queue", &self.in_progress_queue)
            .field("max", &self.max)
            .finish()
    }
}

pub fn new<S>(s: S, amt: usize) -> Buffered<S>
where
    S: Stream,
    S::Item: Future,
{
    Buffered {
        stream: super::fuse::new(s),
        in_progress_queue: FuturesOrdered::new(),
        max: amt,
    }
}

impl<S> Buffered<S>
where
    S: Stream,
    S::Item: Future,
{
    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }

    /// Acquires a mutable pinned reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut<'a>(self: PinMut<'a, Self>) -> PinMut<'a, S> {
        unsafe { PinMut::new_unchecked(&mut PinMut::get_mut(self).stream) }.get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
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

impl<S> Stream for Buffered<S>
where
    S: Stream,
    S::Item: Future,
{
    type Item = <S::Item as Future>::Output;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        // Try to spawn off as many futures as possible by filling up
        // our in_progress_queue of futures.
        while self.in_progress_queue.len() < self.max {
            match self.stream().poll_next(cx) {
                Poll::Ready(Some(future)) => self.in_progress_queue.push(future),
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Attempt to pull the next value from the in_progress_queue
        if let Some(val) = ready!(PinMut::new(self.in_progress_queue()).poll_next(cx)) {
            return Poll::Ready(Some(val))
        }

        // If more values are still coming from the stream, we're not done yet
        if self.stream.is_done() {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
}
