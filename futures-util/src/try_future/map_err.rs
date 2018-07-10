use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};

/// Future for the `map_err` combinator, changing the type of a future.
///
/// This is created by the `Future::map_err` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapErr<Fut, F> {
    future: Fut,
    op: Option<F>,
}

impl<Fut, F> MapErr<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(op: Option<F>);

    /// Creates a new MapErr.
    pub(super) fn new(future: Fut, op: F) -> MapErr<Fut, F> {
        MapErr { future, op: Some(op) }
    }
}

impl<Fut: Unpin, F> Unpin for MapErr<Fut, F> {}

impl<Fut, F, E> Future for MapErr<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> E,
{
    type Output = Result<Fut::Item, E>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let op = self.op().take()
                    .expect("MapErr must not be polled after it returned `Poll::Ready`");
                Poll::Ready(result.map_err(op))
            }
        }
    }
}
