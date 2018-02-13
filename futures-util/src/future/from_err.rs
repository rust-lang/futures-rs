use core::marker::PhantomData;

use anchor_experiment::MovePinned;
use futures_core::{Future, FutureMove, Poll, Async};

/// Future for the `from_err` combinator, changing the error type of a future.
///
/// This is created by the `Future::from_err` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct FromErr<A, E> where A: Future {
    future: A,
    f: PhantomData<E>
}

// Safe because there is only a PhantomData of the error type.
unsafe impl<A: Future + MovePinned, E> MovePinned for FromErr<A, E> where { }

pub fn new<A, E>(future: A) -> FromErr<A, E>
    where A: Future
{
    FromErr {
        future: future,
        f: PhantomData
    }
}

impl<A: Future, E: From<A::Error>> Future for FromErr<A, E> {
    type Item = A::Item;
    type Error = E;

    unsafe fn poll_unsafe(&mut self) -> Poll<A::Item, E> {
        let e = match self.future.poll_unsafe() {
            Ok(Async::Pending) => return Ok(Async::Pending),
            other => other,
        };
        e.map_err(From::from)
    }
}

impl<A: FutureMove, E: From<A::Error>> FutureMove for FromErr<A, E> {
    fn poll_move(&mut self) -> Poll<A::Item, E> {
        let e = match self.future.poll_move() {
            Ok(Async::Pending) => return Ok(Async::Pending),
            other => other,
        };
        e.map_err(From::from)
    }
}
