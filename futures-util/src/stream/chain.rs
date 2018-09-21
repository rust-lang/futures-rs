use core::pin::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

/// An adapter for chaining the output of two streams.
///
/// The resulting stream produces items from first stream and then
/// from second stream.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Chain<St1, St2> {
    first: Option<St1>,
    second: St2,
}

// All interactions with `PinMut<Chain<..>>` happen through these methods
impl<St1, St2> Chain<St1, St2>
where St1: Stream,
      St2: Stream<Item = St1::Item>,
{
    unsafe_pinned!(first: Option<St1>);
    unsafe_pinned!(second: St2);

    pub(super) fn new(stream1: St1, stream2: St2) -> Chain<St1, St2> {
        Chain {
            first: Some(stream1),
            second: stream2,
        }
    }
}

impl<St1, St2> Stream for Chain<St1, St2>
where St1: Stream,
      St2: Stream<Item=St1::Item>,
{
    type Item = St1::Item;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<Self::Item>> {
        if let Some(first) = self.first().as_pin_mut() {
            if let Some(item) = ready!(first.poll_next(cx)) {
                return Poll::Ready(Some(item))
            }
        }
        PinMut::set(self.first(), None);
        self.second().poll_next(cx)
    }
}
