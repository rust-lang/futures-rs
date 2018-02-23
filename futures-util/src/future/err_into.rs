use core::marker::PhantomData;

use futures_core::{Future, Poll, Async};
use futures_core::task;

/// Future for the `err_into` combinator, changing the error type of a future.
///
/// This is created by the `Future::err_into` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ErrInto<A, E> where A: Future {
    future: A,
    f: PhantomData<E>
}

pub fn new<A, E>(future: A) -> ErrInto<A, E>
    where A: Future
{
    ErrInto {
        future: future,
        f: PhantomData
    }
}

impl<A: Future, E> Future for ErrInto<A, E>
    where A::Error: Into<E>
{
    type Item = A::Item;
    type Error = E;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<A::Item, E> {
        let e = match self.future.poll(cx) {
            Ok(Async::Pending) => return Ok(Async::Pending),
            other => other,
        };
        e.map_err(Into::into)
    }
}
