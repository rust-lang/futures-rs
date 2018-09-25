use crate::stream::Fuse;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::marker::Unpin;
use std::mem;
use std::pin::Pin;
use std::prelude::v1::*;

/// An adaptor that chunks up elements in a vector.
///
/// This adaptor will buffer up a list of items in the stream and pass on the
/// vector used for buffering when a specified capacity has been reached. This
/// is created by the `Stream::chunks` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Chunks<St: Stream> {
    stream: Fuse<St>,
    items: Vec<St::Item>,
}

impl<St: Unpin + Stream> Unpin for Chunks<St> {}

impl<St: Stream> Chunks<St> where St: Stream {
    unsafe_unpinned!(items:  Vec<St::Item>);
    unsafe_pinned!(stream: Fuse<St>);

    pub(super) fn new(stream: St, capacity: usize) -> Chunks<St> {
        assert!(capacity > 0);

        Chunks {
            stream: super::Fuse::new(stream),
            items: Vec::with_capacity(capacity),
        }
    }

    fn take(mut self: Pin<&mut Self>) -> Vec<St::Item> {
        let cap = self.items().capacity();
        mem::replace(self.items(), Vec::with_capacity(cap))
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

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }
}

impl<St: Stream> Stream for Chunks<St> {
    type Item = Vec<St::Item>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<Self::Item>> {
        let cap = self.items.capacity();
        loop {
            match ready!(self.stream().poll_next(lw)) {
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
