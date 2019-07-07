use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`unit_error`](super::FutureExt::unit_error) combinator.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct UnitError<Fut> {
    #[pin]
    future: Fut,
}

impl<Fut> UnitError<Fut> {
    pub(super) fn new(future: Fut) -> UnitError<Fut> {
        UnitError { future }
    }
}

impl<Fut: FusedFuture> FusedFuture for UnitError<Fut> {
    fn is_terminated(&self) -> bool { self.future.is_terminated() }
}

impl<Fut, T> Future for UnitError<Fut>
    where Fut: Future<Output = T>,
{
    type Output = Result<T, ()>;

    #[pin_project(self)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<T, ()>> {
        self.future.poll(cx).map(Ok)
    }
}
