use core::marker::Unpin;
use core::mem::PinMut;

use futures_core::{Future, Poll};
use futures_core::task;

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by the `Future::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F>
    where A: Future,
{
    Map {
        future,
        f: Some(f),
    }
}

impl<A: Future, F> Map<A, F> {
    unsafe_pinned!(future -> A);
    unsafe_unpinned!(f -> Option<F>);
}

impl<A: Future + Unpin, F> Unpin for Map<A, F> {}

impl<U, A, F> Future for Map<A, F>
    where A: Future,
          F: FnOnce(A::Output) -> U,
{
    type Output = U;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<U> {
        let e = match self.future().poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = self.f().take().expect("cannot poll Map twice");
        Poll::Ready(f(e))
    }
}
