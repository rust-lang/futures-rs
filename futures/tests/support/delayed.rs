use futures::prelude::*;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::mem::PinMut;

pub struct Delayed<F> {
    future: F,
    polled_before: bool
}

impl<F> Delayed<F> {
    unsafe_pinned!(future: F);
    unsafe_unpinned!(polled_before: bool);
}

impl<F: Future> Future for Delayed<F> {
    type Output = F::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        if *self.polled_before() {
            self.future().poll(cx)
        } else {
            *self.polled_before() = true;
            cx.waker().wake();
            Poll::Pending
        }
    }
}

/// Introduces one `Poll::Pending` before polling the given future
pub fn delayed<F>(future: F) -> Delayed<F>
    where F: Future,
{
    Delayed { future, polled_before: false }
}
