extern crate futures;

use std::marker;

use futures::{Future, Poll, Async, Stream};

enum CoResult<Y,R> {
    Yield(Y),
    Return(R)
}

struct CoFuture<F, T, E> {
    f: F,
    _marker: marker::PhantomData<fn() -> (T, E)>,
}

impl<F, T, E> Future for CoFuture<F, T, E>
    where F: FnMut(()) -> CoResult<(), Result<T, E>>,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        match (self.f)(()) {
            CoResult::Yield(()) => Ok(Async::NotReady),
            CoResult::Return(Ok(t)) => Ok(t.into()),
            CoResult::Return(Err(e)) => Err(e),
        }
    }
}

struct CoStream<F, T, E> {
    f: F,
    _marker: marker::PhantomData<fn() -> (T, E)>,
}

impl<F, T, E> Stream for CoStream<F, T, E>
    where F: FnMut(()) -> CoResult<Option<T>, Result<(), E>>,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<T>, E> {
        match (self.f)(()) {
            CoResult::Yield(Some(t)) => Ok(Some(t).into()),
            CoResult::Yield(None) => Ok(Async::NotReady),
            CoResult::Return(Ok(())) => Ok(None.into()),
            CoResult::Return(Err(e)) => Err(e),
        }
    }
}

fn main() {}
