pub extern crate futures;

use futures::Future;

pub use std::boxed::Box;

pub trait FutureType {
    type Item;
    type Error;
}

pub trait MyFuture<T: FutureType>: Future<Item=T::Item, Error=T::Error> {}

impl<F: Future + ?Sized> MyFuture<Result<F::Item, F::Error>> for F {}

impl<T, E> FutureType for Result<T, E> {
    type Item = T;
    type Error = E;
}
