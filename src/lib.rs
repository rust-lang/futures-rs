#![feature(generator_trait)]

extern crate futures_await_macros; // the compiler lies that this has no effect
extern crate futures;

pub use futures_await_macros::*;
pub use futures::{Future, Stream, Async};
pub use std::result::Result::{Ok, Err};
pub use std::option::Option::{Some, None};
pub use std::boxed::Box;
pub use std::ops::Generator;

use std::ops::State;
use futures::Poll;

// Convenience trait to project from `Result` and get the item/error types
//
// This is how we work with type aliases like `io::Result` without knowing
// whether you're using a type alias.

pub trait FutureType {
    type Item;
    type Error;

    fn into_result(self) -> Result<Self::Item, Self::Error>;
}

impl<T, E> FutureType for Result<T, E> {
    type Item = T;
    type Error = E;

    fn into_result(self) -> Result<Self::Item, Self::Error> {
        self
    }
}

// Random hack for this ICEing in the compiler somewhere
//
//     -> impl Future<Item = <T as FutureType>::Item,
//                    Error = <T as FutureType>::Error>
//
// but also this strategy ICEs elsewhere, so not used yet.
pub trait MyFuture<T: FutureType>: Future<Item=T::Item, Error=T::Error> {}
impl<F: Future + ?Sized> MyFuture<Result<F::Item, F::Error>> for F {}

// Small shim to translate from a generator to a future.

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

// Ye Olde Await Macro
//
// Basically a translation of polling to yielding

#[macro_export]
macro_rules! await {
    ($e:expr) => ({
        let mut future = $e;
        let ret;
        loop {
            match $crate::Future::poll(&mut future) {
                $crate::Ok($crate::Async::Ready(e)) => {
                    ret = $crate::Ok(e);
                    break
                }
                $crate::Ok($crate::Async::NotReady) => yield,
                $crate::Err(e) => {
                    ret = $crate::Err(e);
                    break
                }
            }
        }
        ret
    })
}
