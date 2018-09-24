use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`unwrap_or_else`](super::TryFutureExt::unwrap_or_else)
/// combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct UnwrapOrElse<Fut, F> {
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> UnwrapOrElse<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(f: Option<F>);

    /// Creates a new UnwrapOrElse.
    pub(super) fn new(future: Fut, f: F) -> UnwrapOrElse<Fut, F> {
        UnwrapOrElse { future, f: Some(f) }
    }
}

impl<Fut: Unpin, F> Unpin for UnwrapOrElse<Fut, F> {}

impl<Fut, F> Future for UnwrapOrElse<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> Fut::Ok,
{
    type Output = Fut::Ok;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let op = self.f().take()
                    .expect("UnwrapOrElse already returned `Poll::Ready` before");
                Poll::Ready(result.unwrap_or_else(op))
            }
        }
    }
}
