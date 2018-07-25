use core::fmt;
use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};

/// Future for the [`unwrap`](super::TryFutureExt::unwrap) combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Unwrap<Fut> {
    future: Fut,
}

impl<Fut> Unwrap<Fut> {
    unsafe_pinned!(future: Fut);

    pub(super) fn new(future: Fut) -> Unwrap<Fut> {
        Unwrap { future }
    }
}

impl<Fut: Unpin> Unpin for Unwrap<Fut> {}

impl<Fut, E> Future for Unwrap<Fut>
where Fut: TryFuture<Error = E>,
      E: fmt::Debug
{
    type Output = Fut::Ok;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        self.future().try_poll(cx).map(|output| output.unwrap())
    }
}
