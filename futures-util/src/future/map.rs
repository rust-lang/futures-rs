use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by the `Future::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<Fut, F> {
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> Map<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(f: Option<F>);

    /// Creates a new Map.
    pub(super) fn new(future: Fut, f: F) -> Map<Fut, F> {
        Map { future, f: Some(f) }
    }
}

impl<Fut: Unpin, F> Unpin for Map<Fut, F> {}

impl<Fut, F> FusedFuture for Map<Fut, F> {
    fn is_terminated(&self) -> bool { self.f.is_none() }
}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<T> {
        match self.future().poll(lw) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(output) => {
                let f = self.f().take()
                    .expect("Map must not be polled after it returned `Poll::Ready`");
                Poll::Ready(f(output))
            }
        }
    }
}
