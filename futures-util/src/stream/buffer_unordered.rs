use std::fmt;
use std::marker::Unpin;
use std::mem::PinMut;

use futures_core::{task, Future, Poll, Stream};
use futures_sink::Sink;

use stream::{Fuse, FuturesUnordered};

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible, delivering results as they become available.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they complete. This is created by the
/// `Stream::buffer_unordered` method.
#[must_use = "streams do nothing unless polled"]
pub struct BufferUnordered<S>
where
    S: Stream,
    S::Item: Future,
{
    stream: Fuse<S>,
    in_progress_queue: FuturesUnordered<S::Item>,
    max: usize,
}

impl<S> Unpin for BufferUnordered<S>
where
    S: Stream + fmt::Debug,
    S::Item: Future,
{}

impl<S> fmt::Debug for BufferUnordered<S>
where
    S: Stream + fmt::Debug,
    S::Item: Future,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BufferUnordered")
            .field("stream", &self.stream)
            .field("in_progress_queue", &self.in_progress_queue)
            .field("max", &self.max)
            .finish()
    }
}

pub fn new<S>(s: S, amt: usize) -> BufferUnordered<S>
where
    S: Stream,
    S::Item: Future,
{
    BufferUnordered {
        stream: super::fuse::new(s),
        in_progress_queue: FuturesUnordered::new(),
        max: amt,
    }
}

impl<S> BufferUnordered<S>
where
    S: Stream,
    S::Item: Future,
{
    unsafe_pinned!(stream -> Fuse<S>);
    unsafe_unpinned!(in_progress_queue -> FuturesUnordered<S::Item>);

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

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut<'a>(self: PinMut<'a, Self>) -> PinMut<'a, S> {
        unsafe { PinMut::new_unchecked(&mut PinMut::get_mut_unchecked(self).stream) }.get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }
}

impl<S> Stream for BufferUnordered<S>
where
    S: Stream,
    S::Item: Future,
{
    type Item = <S::Item as Future>::Output;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        // First up, try to spawn off as many futures as possible by filling up
        // our slab of futures.
        while self.in_progress_queue.len() < self.max {
            match self.stream().poll_next(cx) {
                Poll::Ready(Some(future)) => self.in_progress_queue().push(future),
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Attempt to pull the next value from the in_progress_queue
        match PinMut::new(self.in_progress_queue()).poll_next(cx) {
            x @ Poll::Pending | x @ Poll::Ready(Some(_)) => return x,
            Poll::Ready(None) => {}
        }

        // If more values are still coming from the stream, we're not done yet
        if self.stream.is_done() {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S> Sink for BufferUnordered<S>
where
    S: Stream + Sink,
    S::Item: Future,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
