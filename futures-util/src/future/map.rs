use futures_core::{Future, Poll};
use futures_core::task;

#[cfg(feature = "nightly")]
use core::mem::Pin;

/// Future for the `map` combinator, changing the type of a future.
///
/// This is created by the `Future::map` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Map<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F, U>(future: A, f: F) -> Map<A, F>
    where A: Future, F: FnOnce(A::Output) -> U,
{
    Map {
        future: future,
        f: Some(f),
    }
}

#[cfg(feature = "nightly")]
impl<U, A, F> Future for Map<A, F>
    where A: Future, F: FnOnce(A::Output) -> U
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

#[cfg(not(feature = "nightly"))]
impl<U, A, F> Future for Map<A, F>
    where A: Future + ::futures_core::Unpin,
          F: FnOnce(A::Output) -> U,
{
    type Output = U;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<U> {
        let e = match self.future.poll_unpin(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };
        let f = self.f.take().expect("cannot poll Map twice");
        Poll::Ready(f(e))
    }

    unpinned_poll!();
}

#[cfg(not(feature = "nightly"))]
unsafe impl<A, F> ::futures_core::Unpin for Map<A, F> {}
