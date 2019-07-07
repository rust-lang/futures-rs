use super::Chain;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`then`](super::FutureExt::then) method.
#[unsafe_project]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Then<Fut1, Fut2, F> {
    #[pin]
    chain: Chain<Fut1, Fut2, F>,
}

impl<Fut1, Fut2, F> Then<Fut1, Fut2, F>
    where Fut1: Future,
          Fut2: Future,
{
    pub(super) fn new(future: Fut1, f: F) -> Then<Fut1, Fut2, F> {
        Then {
            chain: Chain::new(future, f),
        }
    }
}

impl<Fut1, Fut2, F> FusedFuture for Then<Fut1, Fut2, F> {
    fn is_terminated(&self) -> bool { self.chain.is_terminated() }
}

impl<Fut1, Fut2, F> Future for Then<Fut1, Fut2, F>
    where Fut1: Future,
          Fut2: Future,
          F: FnOnce(Fut1::Output) -> Fut2,
{
    type Output = Fut2::Output;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Fut2::Output> {
        self.chain.poll(cx, |output, f| f(output))
    }
}
