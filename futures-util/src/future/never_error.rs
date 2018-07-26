use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// Future for the `never_error` combinator, turning a `Future` into a `TryFuture`.
///
/// This is created by the `FutureExt::never_error` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct NeverError<Fut> {
    future: Fut,
}

impl<Fut> NeverError<Fut> {
    unsafe_pinned!(future: Fut);

    /// Creates a new NeverError.
    pub(super) fn new(future: Fut) -> NeverError<Fut> {
        NeverError { future }
    }
}

impl<Fut: Unpin> Unpin for NeverError<Fut> {}

impl<Fut, T> Future for NeverError<Fut>
    where Fut: Future<Output = T>,
{
    type Output = Result<T, ()>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<T, ()>> {
        self.future().poll(cx).map(Ok)
    }
}
