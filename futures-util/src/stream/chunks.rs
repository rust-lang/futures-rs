use crate::stream::Fuse;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};
use core::mem;
use core::pin::Pin;
use alloc::vec::Vec;

/// Stream for the [`chunks`](super::StreamExt::chunks) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Chunks<St: Stream> {
    #[pin]
    stream: Fuse<St>,
    items: Vec<St::Item>,
    cap: usize, // https://github.com/rust-lang-nursery/futures-rs/issues/1475
}

impl<St: Stream> Chunks<St> where St: Stream {
    pub(super) fn new(stream: St, capacity: usize) -> Chunks<St> {
        assert!(capacity > 0);

        Chunks {
            stream: super::Fuse::new(stream),
            items: Vec::with_capacity(capacity),
            cap: capacity,
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

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    #[pin_project(self)]
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream.get_pin_mut()
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

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            match ready!(self.stream.as_mut().poll_next(cx)) {
                // Push the item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Some(item) => {
                    self.items.push(item);
                    if self.items.len() >= *self.cap {
                        let cap = *self.cap;
                        return Poll::Ready(Some(mem::replace(self.items, Vec::with_capacity(cap))));
                    }
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                None => {
                    let last = if self.items.is_empty() {
                        None
                    } else {
                        let full_buf = mem::replace(self.items, Vec::new());
                        Some(full_buf)
                    };

                    return Poll::Ready(last);
                }
            }
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Chunks<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
