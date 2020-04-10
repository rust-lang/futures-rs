use crate::stream::Fuse;
use futures_core::stream::{Stream, FusedStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use core::mem;
use core::pin::Pin;
use alloc::vec::Vec;

/// Stream for the [`ready_chunks`](super::StreamExt::ready_chunks) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ReadyChunks<St: Stream> {
    stream: Fuse<St>,
    items: Vec<St::Item>,
    cap: usize, // https://github.com/rust-lang/futures-rs/issues/1475
}

impl<St: Unpin + Stream> Unpin for ReadyChunks<St> {}

impl<St: Stream> ReadyChunks<St> where St: Stream {
    unsafe_unpinned!(items:  Vec<St::Item>);
    unsafe_pinned!(stream: Fuse<St>);

    pub(super) fn new(stream: St, capacity: usize) -> ReadyChunks<St> {
        assert!(capacity > 0);

        ReadyChunks {
            stream: super::Fuse::new(stream),
            items: Vec::with_capacity(capacity),
            cap: capacity,
        }
    }

    fn take(mut self: Pin<&mut Self>) -> Vec<St::Item> {
        let cap = self.cap;
        mem::replace(self.as_mut().items(), Vec::with_capacity(cap))
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
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.stream().get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }
}

impl<St: Stream> Stream for ReadyChunks<St> {
    type Item = Vec<St::Item>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            match self.as_mut().stream().poll_next(cx) {
                // Flush all collected data if underlying stream doesn't contain
                // more ready values
                Poll::Pending => {
                    return if self.items.is_empty() {
                        Poll::Pending
                    } else {
                        Poll::Ready(Some(self.as_mut().take()))
                    }
                }

                // Push the ready item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Poll::Ready(Some(item)) => {
                    self.as_mut().items().push(item);
                    if self.items.len() >= self.cap {
                        return Poll::Ready(Some(self.as_mut().take()))
                    }
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                Poll::Ready(None) => {
                    let last = if self.items.is_empty() {
                        None
                    } else {
                        let full_buf = mem::replace(self.as_mut().items(), Vec::new());
                        Some(full_buf)
                    };

                    return Poll::Ready(last);
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let chunk_len = if self.items.is_empty() { 0 } else { 1 };
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(chunk_len);
        let upper = match upper {
            Some(x) => x.checked_add(chunk_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St: FusedStream> FusedStream for ReadyChunks<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.items.is_empty()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for ReadyChunks<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
