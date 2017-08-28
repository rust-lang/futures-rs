use core::marker::PhantomData;

use {Async, Future, Poll, Never};
use never::InfallibleResultExt;

/// Future that can not fail.
pub trait InfallibleFuture: Future {
    /// Poll a future that can not fail.
    ///
    /// Works similar to `poll`, except that it returns an `Async` value directly
    /// rather than `Poll`.
    fn poll_infallible(&mut self) -> Async<Self::Item>;
}

impl<F: Future<Error=Never>> InfallibleFuture for F {
    fn poll_infallible(&mut self) -> Async<Self::Item> {
        self.poll().infallible()
    }
}

#[derive(Debug)]
pub struct InfallibleCastErr<F, E> {
    future: F,
    phantom: PhantomData<E>
}

impl<F, E> InfallibleCastErr<F, E> {
    pub fn new(future: F) -> InfallibleCastErr<F, E> {
        InfallibleCastErr {
            future: future,
            phantom: PhantomData
        }
    }
}

impl<F: InfallibleFuture, E> Future for InfallibleCastErr<F, E> {
    type Item = F::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<F::Item, E> {
        Ok(self.future.poll_infallible())
    }
}
