use crate::stream::Fuse;
use core::cmp::min;
use core::mem;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`ready_chunks`](super::StreamExt::ready_chunks) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct ReadyChunks<St: Stream, C> {
        #[pin]
        stream: Fuse<St>,
        items: C,
        len: usize,
        cap: usize, // https://github.com/rust-lang/futures-rs/issues/1475
    }
}

impl<St: Stream, C: Default> ReadyChunks<St, C> {
    pub(super) fn new(stream: St, capacity: usize) -> Self {
        assert!(capacity > 0);

        Self {
            stream: super::Fuse::new(stream),
            // Would be better if there were a trait for `with_capacity` and `len`.
            items: C::default(),
            len: 0,
            cap: capacity,
        }
    }

    fn take(mut self: Pin<&mut Self>) -> C {
        let this = self.as_mut().project();
        *this.len = 0;
        mem::take(this.items)
    }

    delegate_access_inner!(stream, St, (.));
}

impl<St: Stream, C: Default + Extend<<St as Stream>::Item>> Stream for ReadyChunks<St, C> {
    type Item = C;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();

        loop {
            match this.stream.as_mut().poll_next(cx) {
                // Flush all collected data if underlying stream doesn't contain
                // more ready values
                Poll::Pending => {
                    return if *this.len == 0 {
                        Poll::Pending
                    } else {
                        Poll::Ready(Some(self.take()))
                    }
                }

                // Push the ready item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Poll::Ready(Some(item)) => {
                    this.items.extend(Some(item));
                    *this.len += 1;
                    if *this.len >= *this.cap {
                        return Poll::Ready(Some(self.take()));
                    }
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                Poll::Ready(None) => {
                    let last = if *this.len == 0 { None } else { Some(self.take()) };

                    return Poll::Ready(last);
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let chunk_len = min(self.len, 1);
        let (lower, upper) = self.stream.size_hint();
        let lower = (lower / self.cap).saturating_add(chunk_len);
        let upper = match upper {
            Some(x) => x.checked_add(chunk_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St: FusedStream, C: Default + Extend<<St as Stream>::Item>> FusedStream
    for ReadyChunks<St, C>
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.len == 0
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, C, Item> Sink<Item> for ReadyChunks<S, C>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
