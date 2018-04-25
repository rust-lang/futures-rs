use core::mem::Pin;

use futures_core::{Async, Poll};
use futures_core::task;

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`AsyncExt::inspect`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<A, F> where A: Async {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Inspect<A, F>
    where A: Async,
          F: FnOnce(&A::Output),
{
    Inspect {
        future: future,
        f: Some(f),
    }
}

impl<A, F> Async for Inspect<A, F>
    where A: Async,
          F: FnOnce(&A::Output),
{
    type Output = A::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<A::Output> {
        let e = match unsafe { pinned_field!(self, future) }.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = unsafe {
            Pin::get_mut(&mut self).f.take().expect("cannot poll Inspect twice")
        };
        f(&e);
        Poll::Ready(e)
    }
}
