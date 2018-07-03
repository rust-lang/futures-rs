use std::mem::{self, PinMut};
use std::marker::Unpin;
use std::prelude::v1::*;

use futures_core::{Poll, Stream};
use futures_core::task;

use stream::Fuse;

/// An adaptor that chunks up elements in a vector.
///
/// This adaptor will buffer up a list of items in the stream and pass on the
/// vector used for buffering when a specified capacity has been reached. This
/// is created by the `Stream::chunks` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Chunks<S>
    where S: Stream
{
    items: Vec<S::Item>,
    stream: Fuse<S>
}

pub fn new<S>(s: S, capacity: usize) -> Chunks<S>
    where S: Stream
{
    assert!(capacity > 0);

    Chunks {
        items: Vec::with_capacity(capacity),
        stream: super::fuse::new(s),
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Chunks<S>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

impl<S> Chunks<S> where S: Stream {
    fn take(mut self: PinMut<Self>) -> Vec<S::Item> {
        let cap = self.items().capacity();
        mem::replace(self.items(), Vec::with_capacity(cap))
    }

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

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }

    unsafe_unpinned!(items ->  Vec<S::Item>);
    unsafe_pinned!(stream -> Fuse<S>);
}

impl<S: Unpin + Stream> Unpin for Chunks<S> {}

impl<S> Stream for Chunks<S>
    where S: Stream
{
    type Item = Vec<<S as Stream>::Item>;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        let cap = self.items.capacity();
        loop {
            match ready!(self.stream().poll_next(cx)) {
                // Push the item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Some(item) => {
                    self.items().push(item);
                    if self.items().len() >= cap {
                        return Poll::Ready(Some(self.take()))
                    }
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                None => {
                    let last = if self.items().is_empty() {
                        None
                    } else {
                        let full_buf = mem::replace(self.items(), Vec::new());
                        Some(full_buf)
                    };

                    return Poll::Ready(last);
                }
            }
        }
    }
}
