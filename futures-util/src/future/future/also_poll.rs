use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

use super::Fuse;

pin_project! {
    /// Future for the [`also_poll`](super::FutureExt::also_poll) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct AlsoPoll<Fut, Also> {
        #[pin] fut: Fut,
        #[pin] also: Fuse<Also>,
    }
}

impl<Fut, Also> AlsoPoll<Fut, Also> {
    pub(super) fn new(fut: Fut, also: Also) -> Self {
        AlsoPoll { fut, also: Fuse::new(also) }
    }
}

impl<Fut, Also> Future for AlsoPoll<Fut, Also>
where
    Fut: Future,
    Also: Future<Output = ()>,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _ = this.also.poll(cx);
        this.fut.poll(cx)
    }
}

impl<Fut, Also> FusedFuture for AlsoPoll<Fut, Also>
where
    Fut: FusedFuture,
    Also: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.fut.is_terminated()
    }
}
