use core::mem::Pin;

use futures_core::{Async, Poll};
use futures_core::task;

/// Async for the `map` combinator, changing the type of a future.
///
/// This is created by the `Async::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<A, F> where A: Async {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Map<A, F>
    where A: Async,
{
    Map {
        future: future,
        f: Some(f),
    }
}

impl<U, A, F> Async for Map<A, F>
    where A: Async,
          F: FnOnce(A::Output) -> U,
{
    type Output = U;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<U> {
        let e = match unsafe { pinned_field!(self, future) }.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = unsafe {
            Pin::get_mut(&mut self).f.take().expect("cannot poll Map twice")
        };
        Poll::Ready(f(e))
    }
}
