use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::{Future, TryFuture, Poll};
use futures_core::task;

/// Future for the `unwrap_or_else` combinator. It unwraps the result, returning
/// the content of the `Ok` as `Output` or if the value is an `Err` then it
/// calls `op` with its value.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct UnwrapOrElse<Fut, F> {
    future: Fut,
    op: Option<F>,
}

impl<Fut, F> UnwrapOrElse<Fut, F> {
    unsafe_pinned!(future -> Fut);
    unsafe_unpinned!(op -> Option<F>);

    /// Creates a new UnwrapOrElse.
    pub fn new(future: Fut, op: F) -> UnwrapOrElse<Fut, F> {
        UnwrapOrElse { future, op: Some(op) }
    }
}

impl<Fut: Unpin, F> Unpin for UnwrapOrElse<Fut, F> {}

impl<Fut, F> Future for UnwrapOrElse<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> Fut::Item,
{
    type Output = Fut::Item;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let op = self.op().take()
                    .expect("UnwrapOrElse already returned `Poll::Ready` before");
                Poll::Ready(result.unwrap_or_else(op))
            }
        }
    }
}
