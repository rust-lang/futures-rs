use std::prelude::v1::*;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};

use futures_core::{Future, PollResult, Poll};
use futures_core::task;

/// Future for the `catch_unwind` combinator.
///
/// This is created by the `Future::catch_unwind` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct CatchUnwind<F> where F: Future {
    future: Option<F>,
}

pub fn new<F>(future: F) -> CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    CatchUnwind {
        future: Some(future),
    }
}

impl<F> Future for CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    type Item = Result<F::Item, F::Error>;
    type Error = Box<Any + Send>;

    fn poll(&mut self, cx: &mut task::Context) -> PollResult<Self::Item, Self::Error> {
        let mut future = self.future.take().expect("cannot poll twice");
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            (future.poll(cx), future)
        }));
        match outcome {
            Err(e) => Poll::Ready(Err(e)),
            Ok((Poll::Pending, future)) => {
                self.future = Some(future);
                Poll::Pending
            },
            Ok((Poll::Ready(r), _)) => Poll::Ready(Ok(r)),
        }
    }
}
