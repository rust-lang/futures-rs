use core::mem::PinMut;

use futures_core::{task, Poll, Stream};
use futures_sink::Sink;

/// A stream which "fuse"s a stream once it's terminated.
///
/// Normally streams can behave unpredictably when used after they have already
/// finished, but `Fuse` continues to return `None` from `poll` forever when
/// finished.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Fuse<S> {
    stream: S,
    done: bool,
}

// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Fuse<S>
    where S: Sink
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}

pub fn new<S: Stream>(s: S) -> Fuse<S> {
    Fuse {
        stream: s,
        done: false,
    }
}

impl<S> Fuse<S> {
    /// Returns whether the underlying stream has finished or not.
    ///
    /// If this method returns `true`, then all future calls to poll are
    /// guaranteed to return `None`. If this returns `false`, then the
    /// underlying stream is still in use.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Acquires a mutable pinned reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut<'a>(self: PinMut<'a, Self>) -> PinMut<'a, S> {
        unsafe { PinMut::map_unchecked(self, |x| &mut x.stream) }
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream
    }

    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(done -> bool);
}

impl<S: Stream> Stream for Fuse<S> {
    type Item = S::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        if *self.done() {
            return Poll::Ready(None);
        }

        let item = ready!(self.stream().poll_next(cx));
        if item.is_none() {
            *self.done() = true;
        }
        Poll::Ready(item)
    }
}
