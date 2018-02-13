use std::prelude::v1::*;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe};

use futures_core::{Future, FutureMove, Poll, Async};

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

    unsafe fn poll_unsafe(&mut self) -> Poll<Self::Item, Self::Error> {
        let res = catch_unwind(|| self.future.poll())?;
        match res {
            Ok(Async::Pending) => Ok(Async::Pending),
            Ok(Async::Ready(t)) => {
                self.future = None;
                Ok(Async::Ready(Ok(t)))
            }
            Err(e) => Ok(Async::Ready(Err(e))),
        }
    }
}

impl<F> FutureMove for CatchUnwind<F>
    where F: FutureMove + UnwindSafe,
{
    fn poll_move(&mut self) -> Poll<Self::Item, Self::Error> {
        unsafe { self.poll_unsafe() }
    }
}
