use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{self, Poll};
use futures_core::never::Never;
use pin_project::{pin_project, unsafe_project};

/// Future for the [`never_error`](super::FutureExt::never_error) combinator.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct NeverError<Fut> {
    #[pin]
    future: Fut,
}

impl<Fut> NeverError<Fut> {
    pub(super) fn new(future: Fut) -> NeverError<Fut> {
        NeverError { future }
    }
}

impl<Fut: FusedFuture> FusedFuture for NeverError<Fut> {
    fn is_terminated(&self) -> bool { self.future.is_terminated() }
}

impl<Fut, T> Future for NeverError<Fut>
    where Fut: Future<Output = T>,
{
    type Output = Result<T, Never>;

    #[pin_project(self)]
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.future.poll(cx).map(Ok)
    }
}
