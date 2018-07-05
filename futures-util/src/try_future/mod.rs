//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

use futures_core::TryFuture;
use futures_sink::Sink;

/* TODO
mod join;
mod select;
pub use self::join::{Join, Join3, Join4, Join5};
pub use self::select::Select;

if_std! {
mod join_all;
mod select_all;
mod select_ok;
pub use self::join_all::{join_all, JoinAll};
pub use self::select_all::{SelectAll, SelectAllNext, select_all};
pub use self::select_ok::{SelectOk, select_ok};
}
*/

// Combinators
mod and_then;
pub use self::and_then::AndThen;

mod err_into;
pub use self::err_into::ErrInto;

mod flatten_sink;
pub use self::flatten_sink::FlattenSink;

mod into_future;
crate use self::into_future::IntoFuture;

mod map_err;
pub use self::map_err::MapErr;

mod map_ok;
pub use self::map_ok::MapOk;

mod or_else;
pub use self::or_else::OrElse;

mod unwrap_or_else;
pub use self::unwrap_or_else::UnwrapOrElse;

// Implementation details
mod try_chain;
crate use self::try_chain::{TryChain, TryChainAction};

impl<F: TryFuture> TryFutureExt for F {}

/// Adapters specific to `Result`-returning futures
pub trait TryFutureExt: TryFuture {
    /// Flatten the execution of this future when the successful result of this
    /// future is a sink.
    ///
    /// This can be useful when sink initialization is deferred, and it is
    /// convenient to work with that sink as if sink was available at the
    /// call site.
    ///
    /// Note that this function consumes this future and returns a wrapped
    /// version of it.
    fn flatten_sink(self) -> FlattenSink<Self, Self::Item>
    where
        Self::Item: Sink<SinkError=Self::Error>,
        Self: Sized,
    {
        flatten_sink::new(self)
    }

    /// Map this future's result to a different type, returning a new future of
    /// the resulting type.
    ///
    /// This function is similar to the `Option::map` or `Iterator::map` where
    /// it will change the type of the underlying future. This is useful to
    /// chain along a computation once a future has been resolved.
    ///
    /// The closure provided will only be called if this future is resolved
    /// successfully. If this future returns an error, panics, or is dropped,
    /// then the closure provided will never be invoked.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// let future = future::ready::<Result<i32, i32>>(Ok(1));
    /// let new_future = future.map_ok(|x| x + 3);
    /// assert_eq!(block_on(new_future), Ok(4));
    /// ```
    ///
    /// Calling `map_ok` on an errored `Future` has no effect:
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// let future = future::ready::<Result<i32, i32>>(Err(1));
    /// let new_future = future.map_ok(|x| x + 3);
    /// assert_eq!(block_on(new_future), Err(1));
    /// ```
    fn map_ok<T, F>(self, op: F) -> MapOk<Self, F>
        where F: FnOnce(Self::Item) -> T,
              Self: Sized,
    {
        MapOk::new(self, op)
    }

    /// Map this future's error to a different error, returning a new future.
    ///
    /// This function is similar to the `Result::map_err` where it will change
    /// the error type of the underlying future. This is useful for example to
    /// ensure that futures have the same error type when used with combinators
    /// like `select` and `join`.
    ///
    /// The closure provided will only be called if this future is resolved
    /// with an error. If this future returns a success, panics, or is
    /// dropped, then the closure provided will never be invoked.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::future;
    /// use futures::prelude::*;
    /// use futures::executor::block_on;
    ///
    /// let future = future::ready::<Result<i32, i32>>(Err(1));
    /// let new_future = future.map_err(|x| x + 3);
    /// assert_eq!(block_on(new_future), Err(4));
    /// ```
    ///
    /// Calling `map_err` on a successful `Future` has no effect:
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::future;
    /// use futures::prelude::*;
    /// use futures::executor::block_on;
    ///
    /// let future = future::ready::<Result<i32, i32>>(Ok(1));
    /// let new_future = future.map_err(|x| x + 3);
    /// assert_eq!(block_on(new_future), Ok(1));
    /// ```
    fn map_err<E, F>(self, op: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E,
              Self: Sized,
    {
        MapErr::new(self, op)
    }

    /// Map this future's error to a new error type using the `Into` trait.
    ///
    /// This function does for futures what `try!` does for `Result`,
    /// by letting the compiler infer the type of the resulting error.
    /// Just as `map_err` above, this is useful for example to ensure
    /// that futures have the same error type when used with
    /// combinators like `select` and `join`.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    ///
    /// let future_with_err_u8 = future::ready::<Result<(), u8>>(Err(1));
    /// let future_with_err_i32 = future_with_err_u8.err_into::<i32>();
    /// ```
    fn err_into<E>(self) -> ErrInto<Self, E>
        where Self: Sized,
              Self::Error: Into<E>
    {
        err_into::new(self)
    }

    /// Execute another future after this one has resolved successfully.
    ///
    /// This function can be used to chain two futures together and ensure that
    /// the final future isn't resolved until both have finished. The closure
    /// provided is yielded the successful result of this future and returns
    /// another value which can be converted into a future.
    ///
    /// Note that because `Result` implements the `IntoFuture` trait this method
    /// can also be useful for chaining fallible and serial computations onto
    /// the end of one future.
    ///
    /// If this future is dropped, panics, or completes with an error then the
    /// provided closure `f` is never called.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future::{self, ReadyFuture};
    ///
    /// let future_of_1 = future::ready::<Result<i32, i32>>(Ok(1));
    /// let future_of_4 = future_of_1.and_then(|x| {
    ///     future::ready(Ok(x + 3))
    /// });
    ///
    /// let future_of_err_1 = future::ready::<Result<i32, i32>>(Err(1));
    /// future_of_err_1.and_then(|_| -> ReadyFuture<Result<(), i32>> {
    ///     panic!("should not be called in case of an error");
    /// });
    /// ```
    fn and_then<Fut, F>(self, async_op: F) -> AndThen<Self, Fut, F>
        where F: FnOnce(Self::Item) -> Fut,
              Fut: TryFuture<Error = Self::Error>,
              Self: Sized,
    {
        AndThen::new(self, async_op)
    }

    /// Execute another future if this one resolves with an error.
    ///
    /// Return a future that passes along this future's value if it succeeds,
    /// and otherwise passes the error to the closure `f` and waits for the
    /// future it returns. The closure may also simply return a value that can
    /// be converted into a future.
    ///
    /// Note that because `Result` implements the `IntoFuture` trait this method
    /// can also be useful for chaining together fallback computations, where
    /// when one fails, the next is attempted.
    ///
    /// If this future is dropped, panics, or completes successfully then the
    /// provided closure `f` is never called.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future::{self, ReadyFuture};
    ///
    /// let future_of_err_1 = future::ready::<Result<i32, i32>>(Err(1));
    /// let future_of_4 = future_of_err_1.or_else(|x| {
    ///     future::ready::<Result<i32, ()>>(Ok(x + 3))
    /// });
    ///
    /// let future_of_1 = future::ready::<Result<i32, i32>>(Ok(1));
    /// future_of_1.or_else(|_| -> ReadyFuture<Result<i32, ()>> {
    ///     panic!("should not be called in case of success");
    /// });
    /// ```
    fn or_else<Fut, F>(self, async_op: F) -> OrElse<Self, Fut, F>
        where F: FnOnce(Self::Error) -> Fut,
              Fut: TryFuture<Item = Self::Item>,
              Self: Sized,
    {
        OrElse::new(self, async_op)
    }

    /* TODO
    /// Waits for either one of two differently-typed futures to complete.
    ///
    /// This function will return a new future which awaits for either this or
    /// the `other` future to complete. The returned future will finish with
    /// both the value resolved and a future representing the completion of the
    /// other work.
    ///
    /// Note that this function consumes the receiving futures and returns a
    /// wrapped version of them.
    ///
    /// Also note that if both this and the second future have the same
    /// success/error type you can use the `Either::split` method to
    /// conveniently extract out the value at the end.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future::{self, Either};
    ///
    /// // A poor-man's join implemented on top of select
    ///
    /// fn join<A, B, E>(a: A, b: B) -> Box<Future<Item=(A::Item, B::Item), Error=E>>
    ///     where A: Future<Error = E> + 'static,
    ///           B: Future<Error = E> + 'static,
    ///           E: 'static,
    /// {
    ///     Box::new(a.select(b).then(|res| -> Box<Future<Item=_, Error=_>> {
    ///         match res {
    ///             Ok(Either::Left((x, b))) => Box::new(b.map(move |y| (x, y))),
    ///             Ok(Either::Right((y, a))) => Box::new(a.map(move |x| (x, y))),
    ///             Err(Either::Left((e, _))) => Box::new(future::err(e)),
    ///             Err(Either::Right((e, _))) => Box::new(future::err(e)),
    ///         }
    ///     }))
    /// }}
    /// ```
    fn select<B>(self, other: B) -> Select<Self, B::Future>
        where B: IntoFuture, Self: Sized
    {
        select::new(self, other.into_future())
    }

    /// Joins the result of two futures, waiting for them both to complete.
    ///
    /// This function will return a new future which awaits both this and the
    /// `other` future to complete. The returned future will finish with a tuple
    /// of both results.
    ///
    /// Both futures must have the same error type, and if either finishes with
    /// an error then the other will be dropped and that error will be
    /// returned.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// let a = future::ok::<i32, i32>(1);
    /// let b = future::ok::<i32, i32>(2);
    /// let pair = a.join(b);
    ///
    /// assert_eq!(block_on(pair), Ok((1, 2)));
    /// # }
    /// ```
    ///
    /// If one or both of the joined `Future`s is errored, the resulting
    /// `Future` will be errored:
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// let a = future::ok::<i32, i32>(1);
    /// let b = future::err::<i32, i32>(2);
    /// let pair = a.join(b);
    ///
    /// assert_eq!(block_on(pair), Err(2));
    /// # }
    /// ```
    fn join<B>(self, other: B) -> Join<Self, B::Future>
        where B: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        let f = join::new(self, other.into_future());
        assert_future::<(Self::Item, B::Item), Self::Error, _>(f)
    }

    /// Same as `join`, but with more futures.
    fn join3<B, C>(self, b: B, c: C) -> Join3<Self, B::Future, C::Future>
        where B: IntoFuture<Error=Self::Error>,
              C: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        join::new3(self, b.into_future(), c.into_future())
    }

    /// Same as `join`, but with more futures.
    fn join4<B, C, D>(self, b: B, c: C, d: D)
                      -> Join4<Self, B::Future, C::Future, D::Future>
        where B: IntoFuture<Error=Self::Error>,
              C: IntoFuture<Error=Self::Error>,
              D: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        join::new4(self, b.into_future(), c.into_future(), d.into_future())
    }

    /// Same as `join`, but with more futures.
    fn join5<B, C, D, E>(self, b: B, c: C, d: D, e: E)
                         -> Join5<Self, B::Future, C::Future, D::Future, E::Future>
        where B: IntoFuture<Error=Self::Error>,
              C: IntoFuture<Error=Self::Error>,
              D: IntoFuture<Error=Self::Error>,
              E: IntoFuture<Error=Self::Error>,
              Self: Sized,
    {
        join::new5(self, b.into_future(), c.into_future(), d.into_future(),
                   e.into_future())
    }
*/

    /// Handle errors generated by this future by converting them into
    /// `Self::Item`.
    ///
    /// Because it can never produce an error, the returned `UnwrapOrElse` future can
    /// conform to any specific `Error` type, including `Never`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// let future = future::ready::<Result<(), &str>>(Err("Boom!"));
    /// let new_future = future.unwrap_or_else(|_| ());
    /// assert_eq!(block_on(new_future), ());
    /// ```
    fn unwrap_or_else<F>(self, op: F) -> UnwrapOrElse<Self, F>
        where Self: Sized,
              F: FnOnce(Self::Error) -> Self::Item
    {
        UnwrapOrElse::new(self, op)
    }

    /// Wraps a `TryFuture` so that it implements `Future`. `TryFuture`s
    /// currently do not implement the `Future` trait due to limitations of
    /// the compiler.
    fn into_future(self) -> IntoFuture<Self>
        where Self: Sized,
    {
        IntoFuture::new(self)
    }
}
