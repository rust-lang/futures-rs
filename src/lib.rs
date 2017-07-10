//! Runtime support for the async/await syntax for futures.
//!
//! This crate serves as a masquerade over the `futures` crate itself,
//! reexporting all of its contents. It's intended that you'll do:
//!
//! ```
//! extern crate futures_await as futures;
//! ```
//!
//! This crate adds a `prelude` module which contains various traits as well as
//! the `async` and `await` macros you'll likely want to use.
//!
//! See the crates's README for more information about usage.

#![feature(generator_trait)]
#![feature(use_extern_macros)]

extern crate futures_async_macro; // the compiler lies that this has no effect
extern crate futures_await_macro;
extern crate futures;

pub use futures::*;

pub mod prelude {
    pub use {Future, Stream, Sink, Poll, Async, AsyncSink, StartSend};
    pub use {IntoFuture};
    pub use futures_async_macro::*; // TODO: can we selectively import just `async`?
    pub use futures_await_macro::await;
}

/// A hidden module that's the "runtime support" for the async/await syntax.
///
/// The `async` attribute and the `await` macro both assume that they can find
/// this module and use its contents. All of their dependencies are defined or
/// reexported here in one way shape or form.
///
/// This module has absolutely not stability at all. All contents may change at
/// any time without notice. Do not use this module in your code if you wish
/// your code to be stable.
#[doc(hidden)]
pub mod __rt {
    pub use std::result::Result::{Ok, Err};
    pub use std::option::Option::{Some, None};
    pub use std::boxed::Box;
    pub use std::ops::Generator;

    use futures::Poll;
    use futures::{Future, Async};
    use std::ops::State;

    /// Convenience trait to project from `Result` and get the item/error types
    ///
    /// This is how we work with type aliases like `io::Result` without knowing
    /// whether you're using a type alias.
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

    /// Random hack for this causing problems in the compiler's typechecking
    /// pass. Ideally this trait and impl would not be needed.
    ///
    /// ```ignore
    /// -> impl Future<Item = <T as FutureType>::Item,
    ///                Error = <T as FutureType>::Error>
    /// ```
    pub trait MyFuture<T: FutureType>: Future<Item=T::Item, Error=T::Error> {}
    impl<F: Future + ?Sized> MyFuture<Result<F::Item, F::Error>> for F {}

    /// Small shim to translate from a generator to a future.
    ///
    /// This is the translation layer from the generator/coroutine protocol to
    /// the futures protocol.
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
