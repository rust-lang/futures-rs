use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::{Future, Poll};
use futures_core::task;

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by the `Future::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<Fut, F> {
    future: Fut,
    op: Option<F>,
}

impl<Fut, F> Map<Fut, F> {
    unsafe_pinned!(future -> Fut);
    unsafe_unpinned!(op -> Option<F>);

    /// Creates a new Map.
    pub(super) fn new(future: Fut, op: F) -> Map<Fut, F> {
        Map { future, op: Some(op) }
    }
}

impl<Fut: Unpin, F> Unpin for Map<Fut, F> {}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<T> {
        match self.future().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(output) => {
                let op = self.op().take()
                    .expect("Map must not be polled after it returned `Poll::Ready`");
                Poll::Ready(op(output))
            }
        }
    }
}
