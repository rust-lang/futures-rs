use crate::stream::{StreamExt, Fuse};
use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from either of the underlying streams as
/// they become available, and the streams are polled in a round-robin fashion.
///
/// When either this stream or the provided one ends, the remaining stream will be polled to
/// completion. The stream that ended will no longer be polled.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Select<St1, St2> {
    stream1: Fuse<St1>,
    stream2: Fuse<St2>,
    flag: bool,
}

impl<St1: Unpin, St2: Unpin> Unpin for Select<St1, St2> {}

impl<St1, St2> Select<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    pub(super) fn new(stream1: St1, stream2: St2) -> Select<St1, St2> {
        Select {
            stream1: stream1.fuse(),
            stream2: stream2.fuse(),
            flag: false,
        }
    }
}

impl<St1, St2> Stream for Select<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    type Item = St1::Item;

    fn poll_next(
        self: PinMut<Self>,
        cx: &mut task::Context
    ) -> Poll<Option<St1::Item>> {
        let Select { flag, stream1, stream2 } =
            unsafe { PinMut::get_mut_unchecked(self) };
        let stream1 = unsafe { PinMut::new_unchecked(stream1) };
        let stream2 = unsafe { PinMut::new_unchecked(stream2) };

        if *flag {
            poll_inner(flag, stream1, stream2, cx)
        } else {
            poll_inner(flag, stream2, stream1, cx)
        }
    }
}

fn poll_inner<St1, St2>(
    flag: &mut bool,
    a: PinMut<St1>,
    b: PinMut<St2>,
    cx: &mut task::Context
) -> Poll<Option<St1::Item>>
    where St1: Stream, St2: Stream<Item = St1::Item>
{
    let a_done = match a.poll_next(cx) {
        Poll::Ready(Some(item)) => return Poll::Ready(Some(item)),
        Poll::Ready(None) => true,
        Poll::Pending => false,
    };

    match b.poll_next(cx) {
        Poll::Ready(Some(item)) => {
            // If the other stream isn't finished yet, give them a chance to
            // go first next time as we pulled something off `b`.
            if !a_done {
                *flag = !*flag;
            }
            Poll::Ready(Some(item))
        }
        Poll::Ready(None) if a_done => Poll::Ready(None),
        Poll::Ready(None) | Poll::Pending => Poll::Pending,
    }
}
