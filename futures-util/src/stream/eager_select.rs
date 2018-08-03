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
/// Will complete as soon as any stream completes
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct EagerSelect<St1, St2> {
    stream1: Fuse<St1>,
    stream2: Fuse<St2>,
    flag: bool,
}

impl<St1: Unpin, St2: Unpin> Unpin for EagerSelect<St1, St2> {}

impl<St1, St2> EagerSelect<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    pub(super) fn new(stream1: St1, stream2: St2) -> EagerSelect<St1, St2> {
        EagerSelect {
            stream1: stream1.fuse(),
            stream2: stream2.fuse(),
            flag: false,
        }
    }
}

impl<St1, St2> Stream for EagerSelect<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    type Item = St1::Item;

    fn poll_next(
        self: PinMut<Self>,
        cx: &mut task::Context
    ) -> Poll<Option<St1::Item>> {
        let EagerSelect { flag, stream1, stream2 } =
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
    if let Poll::Ready(poll_result) = a.poll_next(cx) {
        return Poll::Ready(poll_result)
    }

    match b.poll_next(cx) {
        Poll::Pending => Poll::Pending,
        Poll::Ready(Some(item)) => {
            // give the other stream a chance to go first next time as we pulled something off `b`.
            *flag = !*flag;
            Poll::Ready(Some(item))
        }
        Poll::Ready(None) => Poll::Ready(None),
    }
}
