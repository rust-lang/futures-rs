#![feature(generator_trait)]

extern crate futures_await_macros; // the compiler lies that this has no effect
extern crate futures;

pub use futures_await_macros::*;
pub use futures::{Future, Async};
pub use std::result::Result::{Ok, Err};
pub use std::boxed::Box;

use std::ops::{Generator, State};
use futures::Poll;

pub trait FutureType {
    type Item;
    type Error;

    fn into_result(self) -> Result<Self::Item, Self::Error>;
}

// Right now this doesn't work:
//
//     -> impl Future<Item = <T as FutureType>::Item,
//                    Error = <T as FutureType>::Error>
//
// presumably due to a compiler bug? Try to hack around that for now
pub trait MyFuture<T: FutureType>: Future<Item=T::Item, Error=T::Error> {}
impl<F: Future + ?Sized> MyFuture<Result<F::Item, F::Error>> for F {}

impl<T, E> FutureType for Result<T, E> {
    type Item = T;
    type Error = E;

    fn into_result(self) -> Result<Self::Item, Self::Error> {
        self
    }
}

pub struct GenFuture<T>(T);

pub fn gen<T>(t: T) -> GenFuture<T>
    where T: Generator<Yield = ()>,
          T::Return: FutureType,
{
    GenFuture(t)
}

impl<T> Future for GenFuture<T>
    where T: Generator<Yield = ()>,
          T::Return: FutureType,
{
    type Item = <T::Return as FutureType>::Item;
    type Error = <T::Return as FutureType>::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.0.resume(()) {
            State::Yielded(()) => Ok(Async::NotReady),
            State::Complete(e) => e.into_result().map(Async::Ready),
        }
    }
}

#[inline]
pub fn always_false() -> bool { false }
