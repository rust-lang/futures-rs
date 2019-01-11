use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::unsafe_pinned;

/// A type which converts a `Future` into a `Stream`
/// containing a single element.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct IntoStream<Fut: Future> {
    future: Option<Fut>
}

impl<Fut: Future> IntoStream<Fut> {
    unsafe_pinned!(future: Option<Fut>);

    pub(super) fn new(future: Fut) -> IntoStream<Fut> {
        IntoStream {
            future: Some(future)
        }
    }
}

impl<Fut: Future> Stream for IntoStream<Fut> {
    type Item = Fut::Output;

    fn poll_next(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<Self::Item>> {
        let v = match self.as_mut().future().as_pin_mut() {
            Some(fut) => {
                match fut.poll(lw) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => v
                }
            }
            None => return Poll::Ready(None),
        };

        Pin::set(&mut self.as_mut().future(), None);
        Poll::Ready(Some(v))
    }
}
