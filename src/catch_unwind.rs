use {Future, Poll};

use std::prelude::v1::*;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};

/// Future for the `catch_unwind` combinator.
///
/// This is created by this `Future::catch_uwnind` method.
pub struct CatchUnwind<F> where F: Future {
    future: F,
}

pub fn new<F>(future: F) -> CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    CatchUnwind {
        future: future,
    }
}

impl<F> Future for CatchUnwind<F>
    where F: Future + UnwindSafe,
{
    type Item = Result<F::Item, F::Error>;
    type Error = Box<Any + Send>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match catch_unwind(AssertUnwindSafe(|| self.future.poll())) {
            Ok(Poll::NotReady) => Poll::NotReady,
            Ok(Poll::Ok(v)) => Poll::Ok(Ok(v)),
            Ok(Poll::Err(e)) => Poll::Ok(Err(e)),
            Err(e) => Poll::Err(e),
        }
    }
}

impl<F: Future> Future for AssertUnwindSafe<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<F::Item, F::Error> {
        self.0.poll()
    }
}
