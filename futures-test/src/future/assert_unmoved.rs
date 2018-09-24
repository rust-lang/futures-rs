use futures_core::future::Future;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::marker::Pinned;
use std::pin::Pin;
use std::ptr;

/// Combinator for the
/// [`FutureTestExt::assert_unmoved`](super::FutureTestExt::assert_unmoved)
/// method.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct AssertUnmoved<Fut> {
    future: Fut,
    this_ptr: *const AssertUnmoved<Fut>,
    _pinned: Pinned,
}

impl<Fut> AssertUnmoved<Fut> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(this_ptr: *const Self);

    pub(super) fn new(future: Fut) -> Self {
        Self {
            future,
            this_ptr: ptr::null(),
            _pinned: Pinned,
        }
    }
}

impl<Fut: Future> Future for AssertUnmoved<Fut> {
    type Output = Fut::Output;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Self::Output> {
        let cur_this = &*self as *const Self;
        if self.this_ptr.is_null() {
            // First time being polled
            *self.this_ptr() = cur_this;
        } else {
            assert_eq!(self.this_ptr, cur_this, "Future moved between poll calls");
        }
        self.future().poll(cx)
    }
}

impl<Fut> Drop for AssertUnmoved<Fut> {
    fn drop(&mut self) {
        let cur_this = &*self as *const Self;
        assert_eq!(self.this_ptr, cur_this, "Future moved before drop");
    }
}
