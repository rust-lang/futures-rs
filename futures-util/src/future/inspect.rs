use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`super::FutureExt::inspect`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<Fut, F> where Fut: Future {
    future: Fut,
    op: Option<F>,
}

impl<Fut: Future, F: FnOnce(&Fut::Output)> Inspect<Fut, F> {
    unsafe_pinned!(future -> Fut);
    unsafe_unpinned!(op -> Option<F>);

    pub(super) fn new(future: Fut, op: F) -> Inspect<Fut, F> {
        Inspect {
            future,
            op: Some(op),
        }
    }
}

impl<Fut: Future + Unpin, F> Unpin for Inspect<Fut, F> {}

impl<Fut, F> Future for Inspect<Fut, F>
    where Fut: Future,
          F: FnOnce(&Fut::Output),
{
    type Output = Fut::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Fut::Output> {
        let e = match self.future().poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = self.op().take().expect("cannot poll Inspect twice");
        f(&e);
        Poll::Ready(e)
    }
}
