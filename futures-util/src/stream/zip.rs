use crate::stream::{StreamExt, Fuse};
use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from one or both of the underlying
/// streams as they become available.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Zip<St1: Stream, St2: Stream> {
    stream1: Fuse<St1>,
    stream2: Fuse<St2>,
    queued1: Option<St1::Item>,
    queued2: Option<St2::Item>,
}

impl<St1: Stream + Unpin, St2: Stream + Unpin> Unpin for Zip<St1, St2> {}

impl<St1: Stream, St2: Stream> Zip<St1, St2> {
    unsafe_pinned!(stream1: Fuse<St1>);
    unsafe_pinned!(stream2: Fuse<St2>);
    unsafe_unpinned!(queued1: Option<St1::Item>);
    unsafe_unpinned!(queued2: Option<St2::Item>);

    pub(super) fn new(stream1: St1, stream2: St2) -> Zip<St1, St2> {
        Zip {
            stream1: stream1.fuse(),
            stream2: stream2.fuse(),
            queued1: None,
            queued2: None,
        }
    }
}

impl<St1, St2> FusedStream for Zip<St1, St2>
    where St1: Stream, St2: Stream,
{
    fn is_terminated(&self) -> bool {
        self.stream1.is_terminated() && self.stream2.is_terminated()
    }
}

impl<St1, St2> Stream for Zip<St1, St2>
    where St1: Stream, St2: Stream
{
    type Item = (St1::Item, St2::Item);

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker
    ) -> Poll<Option<Self::Item>> {
        if self.queued1().is_none() {
            match self.stream1().poll_next(lw) {
                Poll::Ready(Some(item1)) => *self.queued1() = Some(item1),
                Poll::Ready(None) | Poll::Pending => {}
            }
        }
        if self.queued2().is_none() {
            match self.stream2().poll_next(lw) {
                Poll::Ready(Some(item2)) => *self.queued2() = Some(item2),
                Poll::Ready(None) | Poll::Pending => {}
            }
        }

        if self.queued1().is_some() && self.queued2().is_some() {
            let pair = (self.queued1().take().unwrap(),
                        self.queued2().take().unwrap());
            Poll::Ready(Some(pair))
        } else if self.stream1().is_done() || self.stream2().is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}
