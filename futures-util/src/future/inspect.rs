use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`super::FutureExt::inspect`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<A, F> where A: Future {
    future: A,
    f: Option<F>,
}



impl<A: Future, F: FnOnce(&A::Output)> Inspect<A, F> {
    unsafe_pinned!(future -> A);
    unsafe_unpinned!(f -> Option<F>);

    pub(super) fn new(future: A, f: F) -> Inspect<A, F> {
        Inspect {
            future,
            f: Some(f),
        }
    }
}

impl<A: Future + Unpin, F> Unpin for Inspect<A, F> {}

impl<A, F> Future for Inspect<A, F>
    where A: Future,
          F: FnOnce(&A::Output),
{
    type Output = A::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<A::Output> {
        let e = match self.future().poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = self.f().take().expect("cannot poll Inspect twice");
        f(&e);
        Poll::Ready(e)
    }
}
