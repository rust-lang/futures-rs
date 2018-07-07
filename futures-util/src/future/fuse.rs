use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

/// A future which "fuses" a future once it's been resolved.
///
/// Normally futures can behave unpredictable once they're used after a future
/// has been resolved, but `Fuse` is always defined to return `Async::Pending`
/// from `poll` after it has resolved successfully or returned an error.
///
/// This is created by the `Future::fuse` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Fuse<A: Future> {
    future: Option<A>,
}

pub fn new<A: Future>(f: A) -> Fuse<A> {
    Fuse {
        future: Some(f),
    }
}

impl<A: Future> Fuse<A> {
    unsafe_pinned!(future -> Option<A>);
}

impl<A: Future> Future for Fuse<A> {
    type Output = A::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<A::Output> {
        // safety: we use this &mut only for matching, not for movement
        let v = match self.future().as_pin_mut() {
            Some(fut) => {
                // safety: this re-pinned future will never move before being dropped
                match fut.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => v
                }
            }
            None => return Poll::Pending,
        };

        PinMut::set(self.future(), None);
        Poll::Ready(v)
    }
}
