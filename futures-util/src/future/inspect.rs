use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`inspect`](super::FutureExt::inspect) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Inspect<Fut, F> where Fut: Future {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut: Future, F: FnOnce(&Fut::Output)> Inspect<Fut, F> {
    pub(super) fn new(future: Fut, f: F) -> Inspect<Fut, F> {
        Inspect {
            future,
            f: Some(f),
        }
    }
}

impl<Fut: Future + FusedFuture, F> FusedFuture for Inspect<Fut, F> {
    fn is_terminated(&self) -> bool { self.future.is_terminated() }
}

impl<Fut, F> Future for Inspect<Fut, F>
    where Fut: Future,
          F: FnOnce(&Fut::Output),
{
    type Output = Fut::Output;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Fut::Output> {
        let e = ready!(self.future.as_mut().poll(cx));
        let f = self.f.take().expect("cannot poll Inspect twice");
        f(&e);
        Poll::Ready(e)
    }
}
