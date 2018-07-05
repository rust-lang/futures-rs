use core::mem::PinMut;
use futures_core::{Future, TryFuture};
use futures_core::task::{Context, Poll};

/// Converts a `TryFuture` into a normal `Future`
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct IntoFuture<Fut> {
    future: Fut,
}

impl<Fut> IntoFuture<Fut> {
    unsafe_pinned!(future -> Fut);

    #[inline]
    pub fn new(future: Fut) -> IntoFuture<Fut> {
        IntoFuture { future }
    }
}

impl<Fut: TryFuture> Future for IntoFuture<Fut> {
    type Output = Result<Fut::Item, Fut::Error>;

    #[inline]
    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.future().try_poll(cx)
    }
}
