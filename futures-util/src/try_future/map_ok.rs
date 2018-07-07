use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{Context, Poll};

/// Future for the `map_ok` combinator, changing the type of a future.
///
/// This is created by the `Future::map_ok` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapOk<Fut, F> {
    future: Fut,
    op: Option<F>,
}

impl<Fut, F> MapOk<Fut, F> {
    unsafe_pinned!(future -> Fut);
    unsafe_unpinned!(op -> Option<F>);

    /// Creates a new MapOk.
    pub(super) fn new(future: Fut, op: F) -> MapOk<Fut, F> {
        MapOk { future, op: Some(op) }
    }
}

impl<Fut: Unpin, F> Unpin for MapOk<Fut, F> {}

impl<Fut, F, T> Future for MapOk<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Item) -> T,
{
    type Output = Result<T, Fut::Error>;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let op = self.op().take()
                    .expect("MapOk must not be polled after it returned `Poll::Ready`");
                Poll::Ready(result.map(op))
            }
        }
    }
}
