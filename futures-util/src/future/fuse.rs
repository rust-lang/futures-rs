use core::pin::Pin;
use futures_core::future::{Future, FusedFuture};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::unsafe_pinned;

/// A future which "fuses" a future once it has been resolved.
/// This wrapper can be used to turn any `Future` into a `FusedFuture`.
///
/// Normally, `poll`ing a future after it has completed (returned `Poll::Ready`
/// from a call to `poll`) can cause arbitrary behavior (panics, deadlock).
/// `Fuse` is always defined to return `Poll::Pending` from `poll` after it has
/// resolved.
///
/// This is created by the `Future::fuse` method.
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

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Fut::Output> {
        // safety: we use this &mut only for matching, not for movement
        let v = match self.as_mut().future().as_pin_mut() {
            Some(fut) => {
                // safety: this re-pinned future will never move before being dropped
                match fut.poll(lw) {
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
