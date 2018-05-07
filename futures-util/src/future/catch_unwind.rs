use std::prelude::v1::*;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};

use futures_core::{Future, Poll};
use futures_core::task;

#[cfg(feature = "nightly")]
use std::mem::Pin;

/// Future for the `catch_unwind` combinator.
///
/// This is created by the `Future::catch_unwind` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct CatchUnwind<F> {
    future: F,
}

pub fn new<F>(future: F) -> CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    CatchUnwind { future }
}

#[cfg(feature = "nightly")]
impl<F> Future for CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    type Output = Result<F::Output, Box<Any + Send>>;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let fut = unsafe { pinned_field!(self, future) };
        match catch_unwind(AssertUnwindSafe(|| fut.poll(cx))) {
            Ok(res) => res.map(Ok),
            Err(e) => Poll::Ready(Err(e))
        }
    }
}

#[cfg(not(feature = "nightly"))]
unpinned! {
    impl<F> Future for CatchUnwind<F>
        where F: Future + UnwindSafe + ::futures_core::Unpin,
    {
        type Output = Result<F::Output, Box<Any + Send>>;

        fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output> {
            let fut = &mut self.future;
            match catch_unwind(AssertUnwindSafe(|| fut.poll_unpin(cx))) {
                Ok(res) => res.map(Ok),
                Err(e) => Poll::Ready(Err(e))
            }
        }
    }
}
