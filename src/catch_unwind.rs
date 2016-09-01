use {Future, Poll};

use std::prelude::v1::*;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};

/// Future for the `catch_unwind` combinator.
///
/// This is created by this `Future::catch_uwnind` method.
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

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut future = self.future.take().expect("cannot poll twice");
        match catch_unwind(|| (future.poll(), future)) {
            Ok((Poll::NotReady, f)) => {
                self.future = Some(f);
                Poll::NotReady
            }
            Ok((Poll::Ok(v), _)) => Poll::Ok(Ok(v)),
            Ok((Poll::Err(e), _)) => Poll::Ok(Err(e)),
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
