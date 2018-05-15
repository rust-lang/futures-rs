use core::mem::PinMut;

use futures_core::{Future, Poll};
use futures_core::task;

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`FutureExt::inspect`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<A, F> where A: Future {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Inspect<A, F>
    where A: Future,
          F: FnOnce(&A::Output),
{
    Inspect {
        future: future,
        f: Some(f),
    }
}

impl<A, F> Future for Inspect<A, F>
    where A: Future,
          F: FnOnce(&A::Output),
{
    type Output = A::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<A::Output> {
        let e = match unsafe { pinned_field!(self, future) }.poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = unsafe {
            PinMut::get_mut(self).f.take().expect("cannot poll Inspect twice")
        };
        f(&e);
        Poll::Ready(e)
    }
}
