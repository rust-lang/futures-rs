use crate::stream::{Fuse, IntoStream, StreamExt};

use core::cmp::min;
use core::pin::Pin;
use core::{fmt, mem};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_chunks`](super::TryStreamExt::try_chunks) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryChunks<St: TryStream, C> {
        #[pin]
        stream: Fuse<IntoStream<St>>,
        items: C,
        len: usize,
        cap: usize, // https://github.com/rust-lang/futures-rs/issues/1475
    }
}

impl<St: TryStream, C: Default> TryChunks<St, C> {
    pub(super) fn new(stream: St, capacity: usize) -> Self {
        assert!(capacity > 0);

        Self {
            stream: IntoStream::new(stream).fuse(),
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

    delegate_access_inner!(stream, St, (. .));
}

type TryChunksStreamError<C, St> = TryChunksError<C, <St as TryStream>::Error>;

impl<St: TryStream, C: Default + Extend<<St as TryStream>::Ok>> Stream for TryChunks<St, C> {
    type Item = Result<C, TryChunksStreamError<C, St>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        loop {
            match ready!(this.stream.as_mut().try_poll_next(cx)) {
                // Push the item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Some(item) => match item {
                    Ok(item) => {
                        this.items.extend(Some(item));
                        *this.len += 1;
                        if *this.len >= *this.cap {
                            return Poll::Ready(Some(Ok(self.take())));
                        }
                    }
                    Err(e) => {
                        return Poll::Ready(Some(Err(TryChunksError(self.take(), e))));
                    }
                },

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                None => {
                    let last = if *this.len == 0 { None } else { Some(self.take()) };

                    return Poll::Ready(last.map(Ok));
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

impl<St: TryStream + FusedStream, C: Default + Extend<<St as TryStream>::Ok>> FusedStream
    for TryChunks<St, C>
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() && self.len == 0
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, C, Item> Sink<Item> for TryChunks<S, C>
where
    S: TryStream + Sink<Item>,
{
    type Error = <S as Sink<Item>>::Error;

    delegate_sink!(stream, Item);
}

/// Error indicating, that while chunk was collected inner stream produced an error.
///
/// Contains all items that were collected before an error occurred, and the stream error itself.
#[derive(PartialEq, Eq)]
pub struct TryChunksError<C, E>(pub C, pub E);

impl<C, E: fmt::Debug> fmt::Debug for TryChunksError<C, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

impl<C, E: fmt::Display> fmt::Display for TryChunksError<C, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<C, E: fmt::Debug + fmt::Display> std::error::Error for TryChunksError<C, E> {}
