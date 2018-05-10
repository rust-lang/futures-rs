use futures_core::{Future, Poll};
use futures_core::task;

#[cfg(feature = "nightly")]
use core::mem::PinMut;

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`FutureExt::inspect`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<A, F> {
    future: A,
    f: Option<F>,
}

pub fn new<A, F>(future: A, f: F) -> Inspect<A, F> {
    Inspect {
        future: future,
        f: Some(f),
    }
}

#[cfg(feature = "nightly")]
impl<A, F> Future for Inspect<A, F>
    where A: Future, F: FnOnce(&A::Output)
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

#[cfg(not(feature = "nightly"))]
impl<A, F> Future for Inspect<A, F>
    where A: Future + ::futures_core::Unpin,
          F: FnOnce(&A::Output)
{
    type Output = A::Output;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<A::Output> {
        let e = match self.future.poll_unpin(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };
        let f = self.f.take().expect("cannot poll Inspect twice");
        f(&e);
        Poll::Ready(e)
    }

    unpinned_poll!();
}

#[cfg(not(feature = "nightly"))]
unsafe impl<A, F> ::futures_core::Unpin for Inspect<A, F> {}
