use core::pin::Pin;
use futures_core::future::{Future, FusedFuture};
use futures_core::task::{Context, Poll};
use pin_utils::unsafe_pinned;

/// Future for the [`fuse`](super::FutureExt::fuse) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Fuse<Fut: Future> {
    future: Option<Fut>,
}

impl<Fut: Future> Fuse<Fut> {
    unsafe_pinned!(future: Option<Fut>);

    pub(super) fn new(f: Fut) -> Fuse<Fut> {
        Fuse {
            future: Some(f),
        }
    }
}

impl<Fut: Future> FusedFuture for Fuse<Fut> {
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}

impl<Fut: Future> Future for Fuse<Fut> {
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Fut::Output> {
        // safety: we use this &mut only for matching, not for movement
        let v = match self.as_mut().future().as_pin_mut() {
            Some(fut) => {
                // safety: this re-pinned future will never move before being dropped
                match fut.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => v
                }
            }
            None => return Poll::Pending,
        };

        Pin::set(&mut self.as_mut().future(), None);
        Poll::Ready(v)
    }
}
