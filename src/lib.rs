#![feature(generator_trait)]
#![feature(use_extern_macros)]

extern crate futures_async_macro; // the compiler lies that this has no effect
extern crate futures_await_macro;
extern crate futures;

pub use futures::*;

pub mod prelude {
    pub use {Future, Stream, Sink, Poll, Async, AsyncSink, StartSend};
    pub use futures_async_macro::*;
    pub use futures_await_macro::await;
}

pub mod __rt {

    pub use std::result::Result::{Ok, Err};
    pub use std::option::Option::{Some, None};
    pub use std::boxed::Box;
    pub use std::ops::Generator;

    use futures::Poll;
    use futures::{Future, Async};
    use std::ops::State;

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
}
