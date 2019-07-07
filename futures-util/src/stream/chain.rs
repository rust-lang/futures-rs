use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Stream for the [`chain`](super::StreamExt::chain) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Chain<St1, St2> {
    #[pin]
    first: Option<St1>,
    #[pin]
    second: St2,
}

// All interactions with `Pin<&mut Chain<..>>` happen through these methods
impl<St1, St2> Chain<St1, St2>
where St1: Stream,
      St2: Stream<Item = St1::Item>,
{
    pub(super) fn new(stream1: St1, stream2: St2) -> Chain<St1, St2> {
        Chain {
            first: Some(stream1),
            second: stream2,
        }
    }
}

impl<St1, St2: FusedStream> FusedStream for Chain<St1, St2> {
    fn is_terminated(&self) -> bool {
        self.first.is_none() && self.second.is_terminated()
    }
}

impl<St1, St2> Stream for Chain<St1, St2>
where St1: Stream,
      St2: Stream<Item=St1::Item>,
{
    type Item = St1::Item;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if let Some(first) = self.first.as_mut().as_pin_mut() {
            if let Some(item) = ready!(first.poll_next(cx)) {
                return Poll::Ready(Some(item))
            }
        }
        self.first.set(None);
        self.second.poll_next(cx)
    }
}
