use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

/// Creates a stream of single element
///
/// ```
/// use futures::future;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let mut stream = stream::once(future::ready(17));
/// let collected = block_on(stream.collect::<Vec<i32>>());
/// assert_eq!(collected, vec![17]);
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
        cx: &mut task::Context,
    ) -> Poll<Option<Fut::Output>> {
        let val = if let Some(f) = self.future().as_pin_mut() {
            ready!(f.poll(cx))
        } else {
            return Poll::Ready(None)
        };
        Pin::set(self.future(), None);
        Poll::Ready(Some(val))
    }
}
