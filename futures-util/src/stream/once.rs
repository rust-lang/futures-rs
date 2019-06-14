use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_utils::unsafe_pinned;

/// Creates a stream of a single element.
///
/// ```
/// #![feature(async_await)]
/// # futures::executor::block_on(async {
/// use futures::future;
/// use futures::stream::{self, StreamExt};
///
/// let stream = stream::once(future::ready(17));
/// let collected = stream.collect::<Vec<i32>>().await;
/// assert_eq!(collected, vec![17]);
/// # });
/// ```
pub fn once<Fut: Future>(future: Fut) -> Once<Fut> {
    Once { future: Some(future) }
}

/// A stream which emits single element and then EOF.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Once<Fut> {
    future: Option<Fut>
}

impl<Fut: Unpin> Unpin for Once<Fut> {}

impl<Fut> Once<Fut> {
    unsafe_pinned!(future: Option<Fut>);
}

impl<Fut: Future> Stream for Once<Fut> {
    type Item = Fut::Output;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Fut::Output>> {
        let val = if let Some(f) = self.as_mut().future().as_pin_mut() {
            ready!(f.poll(cx))
        } else {
            return Poll::Ready(None)
        };
        self.future().set(None);
        Poll::Ready(Some(val))
    }
}
