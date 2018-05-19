use core::mem::PinMut;

use futures_core::{Poll, Stream};
use futures_core::task;

use stream::{StreamExt, Fuse};

/// An adapter for merging the output of two streams.
///
/// The merged stream produces items from either of the underlying streams as
/// they become available, and the streams are polled in a round-robin fashion.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Select<S1, S2> {
    stream1: Fuse<S1>,
    stream2: Fuse<S2>,
    flag: bool,
}

pub fn new<S1, S2>(stream1: S1, stream2: S2) -> Select<S1, S2>
    where S1: Stream,
          S2: Stream<Item = S1::Item>
{
    Select {
        stream1: stream1.fuse(),
        stream2: stream2.fuse(),
        flag: false,
    }
}

impl<S1, S2> Select<S1, S2> {
    unsafe_unpinned!(flag -> bool);

    fn project<'a>(self: PinMut<'a, Self>) -> (&'a mut bool, PinMut<'a, Fuse<S1>>, PinMut<'a, Fuse<S2>>) {
        unsafe {
            let Select { stream1, stream2, flag } = PinMut::get_mut(self);
            (flag, PinMut::new_unchecked(stream1), PinMut::new_unchecked(stream2))
        }
    }
}

fn poll_inner<S1, S2>(flag: &mut bool, a: PinMut<S1>, b: PinMut<S2>, cx: &mut task::Context)
    -> Poll<Option<S1::Item>>
where S1: Stream, S2: Stream<Item = S1::Item>
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

impl<S1, S2> Stream for Select<S1, S2>
    where S1: Stream,
          S2: Stream<Item = S1::Item>
{
    type Item = S1::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S1::Item>> {
        let flipped = *self.flag();
        let (flag, s1, s2) = self.project();

        if flipped {
            poll_inner(flag, s1, s2, cx)
        } else {
            poll_inner(flag, s2, s1, cx)
        }
    }
}
