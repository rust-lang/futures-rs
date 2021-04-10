use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::pin_project;
use std::pin::Pin;

/// Combinator that guarantees one [`Poll::Pending`] before polling its inner
/// future.
///
/// This is created by the
/// [`FutureTestExt::pending_once`](super::FutureTestExt::pending_once)
/// method.
#[pin_project]
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PendingOnce<Fut> {
    #[pin]
    future: Fut,
    polled_before: bool,
}

impl<Fut: Future> PendingOnce<Fut> {
    pub(super) fn new(future: Fut) -> Self {
        Self { future, polled_before: false }
    }
}

impl<Fut: Future> Future for PendingOnce<Fut> {
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if *this.polled_before {
            this.future.poll(cx)
        } else {
            *this.polled_before = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl<Fut: FusedFuture> FusedFuture for PendingOnce<Fut> {
    fn is_terminated(&self) -> bool {
        self.polled_before && self.future.is_terminated()
    }
}
