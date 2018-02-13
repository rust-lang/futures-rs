use core::marker::PhantomData;

use futures_core::{Future, Poll, Async};
use futures_core::task;

/// Future for the `from_err` combinator, changing the error type of a future.
///
/// This is created by the `Future::from_err` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct FromErr<A, E> where A: Future {
    future: A,
    f: PhantomData<E>
}

pub fn new<A, E>(future: A) -> FromErr<A, E>
    where A: Future
{
    FromErr {
        future: future,
        f: PhantomData
    }
}

impl<A:Future, E:From<A::Error>> Future for FromErr<A, E> {
    type Item = A::Item;
    type Error = E;

    fn poll(&mut self, ctx: &mut task::Context) -> Poll<A::Item, E> {
        let e = match self.future.poll(ctx) {
            Ok(Async::Pending) => return Ok(Async::Pending),
            other => other,
        };
        e.map_err(From::from)
    }
}
