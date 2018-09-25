use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`map_err`](super::TryFutureExt::map_err) combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapErr<Fut, F> {
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> MapErr<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(f: Option<F>);

    /// Creates a new MapErr.
    pub(super) fn new(future: Fut, f: F) -> MapErr<Fut, F> {
        MapErr { future, f: Some(f) }
    }
}

impl<Fut: Unpin, F> Unpin for MapErr<Fut, F> {}

impl<Fut, F, E> Future for MapErr<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> E,
{
    type Output = Result<Fut::Ok, E>;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Self::Output> {
        match self.future().try_poll(lw) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let f = self.f().take()
                    .expect("MapErr must not be polled after it returned `Poll::Ready`");
                Poll::Ready(result.map_err(f))
            }
        }
    }
}
