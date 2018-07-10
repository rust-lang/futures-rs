use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// An adapter for chaining the output of two streams.
///
/// The resulting stream produces items from first stream and then
/// from second stream.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Chain<S1, S2> {
    first: Option<S1>,
    second: S2,
}

pub fn new<S1, S2>(s1: S1, s2: S2) -> Chain<S1, S2>
    where S1: Stream, S2: Stream<Item=S1::Item>,
{
    Chain {
        first: Some(s1),
        second: s2,
    }
}

// All interactions with `PinMut<Chain<..>>` happen through these methods
impl<S1, S2> Chain<S1, S2> {
    unsafe_pinned!(first: Option<S1>);
    unsafe_pinned!(second: S2);
}

impl<S1, S2> Stream for Chain<S1, S2>
    where S1: Stream, S2: Stream<Item=S1::Item>,
{
    type Item = S1::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        if let Some(first) = self.first().as_pin_mut() {
            if let Some(item) = ready!(first.poll_next(cx)) {
                return Poll::Ready(Some(item))
            }
        }
        PinMut::set(self.first(), None);
        self.second().poll_next(cx)
    }
}
