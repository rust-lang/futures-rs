use futures_core::future::Future;
use futures_core::task::{self, Poll};
use std::any::Any;
use std::mem::PinMut;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};
use std::prelude::v1::*;

/// Future for the `catch_unwind` combinator.
///
/// This is created by the `Future::catch_unwind` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct CatchUnwind<F> where F: Future {
    future: F,
}

impl<F> CatchUnwind<F> where F: Future + UnwindSafe {
    unsafe_pinned!(future -> F);

    pub(super) fn new(future: F) -> CatchUnwind<F> {
        CatchUnwind { future }
    }
}

impl<F> Future for CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    type Output = Result<F::Output, Box<dyn Any + Send>>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match catch_unwind(AssertUnwindSafe(|| self.future().poll(cx))) {
            Ok(res) => res.map(Ok),
            Err(e) => Poll::Ready(Err(e))
        }
    }
}
