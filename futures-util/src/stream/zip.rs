use crate::stream::{StreamExt, Fuse};
use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from one or both of the underlying
/// streams as they become available. Errors, however, are not merged: you
#[derive(Debug)]
/// get at most one error at a time.
#[must_use = "streams do nothing unless polled"]
pub struct Zip<S1: Stream, S2: Stream> {
    stream1: Fuse<S1>,
    stream2: Fuse<S2>,
    queued1: Option<S1::Item>,
    queued2: Option<S2::Item>,
}

pub fn new<S1, S2>(stream1: S1, stream2: S2) -> Zip<S1, S2>
    where S1: Stream, S2: Stream
{
    Zip {
        stream1: stream1.fuse(),
        stream2: stream2.fuse(),
        queued1: None,
        queued2: None,
    }
}

impl<S1: Stream, S2: Stream> Zip<S1, S2> {
    unsafe_pinned!(stream1: Fuse<S1>);
    unsafe_pinned!(stream2: Fuse<S2>);
    unsafe_unpinned!(queued1: Option<S1::Item>);
    unsafe_unpinned!(queued2: Option<S2::Item>);
}

impl<S1: Unpin + Stream, S2: Unpin + Stream> Unpin for Zip<S1, S2> {}

impl<S1, S2> Stream for Zip<S1, S2>
    where S1: Stream, S2: Stream
{
    type Item = (S1::Item, S2::Item);

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        if self.queued1().is_none() {
            match self.stream1().poll_next(cx) {
                Poll::Ready(Some(item1)) => *self.queued1() = Some(item1),
                Poll::Ready(None) | Poll::Pending => {}
            }
        }
        if self.queued2().is_none() {
            match self.stream2().poll_next(cx) {
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
