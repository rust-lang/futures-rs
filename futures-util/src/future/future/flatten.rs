use core::fmt::{self, Debug};
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::pin_project;

#[pin_project]
#[derive(Debug)]
enum InternalFlatten<Fut: Future> {
    First(#[pin] Fut),
    Second(#[pin] Fut::Output),
    Empty,
}

impl<Fut: Future> InternalFlatten<Fut> {
    fn new(future: Fut) -> Self {
        InternalFlatten::First(future)
    }
}

impl<Fut> FusedFuture for InternalFlatten<Fut>
    where Fut: Future,
          Fut::Output: Future,
{
    fn is_terminated(&self) -> bool {
        match self {
            InternalFlatten::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Future for InternalFlatten<Fut>
    where Fut: Future,
          Fut::Output: Future,
{
    type Output = <Fut::Output as Future>::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(loop {
            match self.as_mut().project() {
                __InternalFlattenProjection::First(f) => {
                    let f = ready!(f.poll(cx));
                    self.set(InternalFlatten::Second(f));
                },
                __InternalFlattenProjection::Second(f) => {
                    let output = ready!(f.poll(cx));
                    self.set(InternalFlatten::Empty);
                    break output;
                },
                __InternalFlattenProjection::Empty => unreachable!()
            }
        })
    }
}

/// Future for the [`flatten`](super::FutureExt::flatten) method.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project]
pub struct Flatten<Fut: Future>(#[pin] InternalFlatten<Fut>);

impl<Fut: Debug + Future> Debug for Flatten<Fut> where Fut::Output: Debug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<Fut: Future> Flatten<Fut> {
    pub(super) fn new(future: Fut) -> Self {
        Self(InternalFlatten::new(future))
    }
}

impl<Fut: Future> FusedFuture for Flatten<Fut> where Fut::Output: Future {
    fn is_terminated(&self) -> bool { self.0.is_terminated() }
}

impl<Fut: Future> Future for Flatten<Fut> where Fut::Output: Future {
    type Output = <Fut::Output as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}
