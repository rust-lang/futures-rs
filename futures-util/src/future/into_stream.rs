use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_utils::unsafe_pinned;

/// Stream for the [`into_stream`](super::FutureExt::into_stream) method.
#[must_use = "streams do nothing unless polled"]
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

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let v = match self.as_mut().future().as_pin_mut() {
            Some(fut) => ready!(fut.poll(cx)),
            None => return Poll::Ready(None),
        };

        self.as_mut().future().set(None);
        Poll::Ready(Some(v))
    }
}
