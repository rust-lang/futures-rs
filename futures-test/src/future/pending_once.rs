use futures_core::future::Future;
use futures_core::task::{self, Poll};
use std::mem::PinMut;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Combinator that guarantees one [`Poll::Pending`] before polling its inner
/// future.
///
/// This is created by the
/// [`FutureTestExt::pending_once`](super::FutureTestExt::pending_once)
/// method.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct PendingOnce<Fut: Future> {
    future: Fut,
    polled_before: bool,
}

impl<Fut: Future> PendingOnce<Fut> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(polled_before: bool);

    pub(super) fn new(future: Fut) -> Self {
        Self {
            future,
            polled_before: false,
        }
    }
}

impl<Fut: Future> Future for PendingOnce<Fut> {
    type Output = Fut::Output;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        if *self.polled_before() {
            self.future().poll(cx)
        } else {
            *self.polled_before() = true;
            cx.waker().wake();
            Poll::Pending
        }
    }
}
