use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};

/// Future for the `unwrap_or_else` combinator. It unwraps the result, returning
/// the content of the `Ok` as `Output` or if the value is an `Err` then it
/// calls `op` with its value.
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
        mut self: PinMut<Self>,
        cx: &mut task::Context,
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
