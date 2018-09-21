use core::pin::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

/// Future for the [`into_future`](super::TryFutureExt::into_future) combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct IntoFuture<Fut> {
    future: Fut,
}

impl<Fut> IntoFuture<Fut> {
    unsafe_pinned!(future: Fut);

    #[inline]
    pub(super) fn new(future: Fut) -> IntoFuture<Fut> {
        IntoFuture { future }
    }
}

impl<Fut: TryFuture> Future for IntoFuture<Fut> {
    type Output = Result<Fut::Ok, Fut::Error>;

    #[inline]
    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        self.future().try_poll(cx)
    }
}
