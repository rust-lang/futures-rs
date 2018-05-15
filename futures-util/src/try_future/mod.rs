//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

use futures_core::TryFuture;

// combinators
mod and_then;
mod map_ok;
mod map_err;
mod err_into;
mod or_else;
mod recover;

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

pub use self::and_then::AndThen;
pub use self::map_ok::MapOk;
pub use self::map_err::MapErr;
pub use self::err_into::ErrInto;
pub use self::or_else::OrElse;
pub use self::recover::Recover;

impl<F: TryFuture> TryFutureExt for F {}

/// Adapters specific to `Result`-returning futures
pub trait TryFutureExt: TryFuture {
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
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::ok::<u32, u32>(1);
    /// let new_future = future.map(|x| x + 3);
    /// assert_eq!(block_on(new_future), Ok(4));
    /// # }
    /// ```
    ///
    /// Calling `map` on an errored `Future` has no effect:
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::err::<u32, u32>(1);
    /// let new_future = future.map(|x| x + 3);
    /// assert_eq!(block_on(new_future), Err(1));
    /// # }
    /// ```
    fn map_ok<U, F>(self, f: F) -> MapOk<Self, F>
        where F: FnOnce(Self::Item) -> U,
              Self: Sized,
    {
        map_ok::new(self, f)
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
    /// # extern crate futures_executor;
    /// use futures::future::err;
    /// use futures::prelude::*;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let future = err::<u32, u32>(1);
    /// let new_future = future.map_err(|x| x + 3);
    /// assert_eq!(block_on(new_future), Err(4));
    /// # }
    /// ```
    ///
    /// Calling `map_err` on a successful `Future` has no effect:
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::future::ok;
    /// use futures::prelude::*;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let future = ok::<u32, u32>(1);
    /// let new_future = future.map_err(|x| x + 3);
    /// assert_eq!(block_on(new_future), Ok(1));
    /// # }
    /// ```
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E,
              Self: Sized,
    {
        map_err::new(self, f)
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
    /// # fn main() {
    /// let future_with_err_u8 = future::err::<(), u8>(1);
    /// let future_with_err_u32 = future_with_err_u8.err_into::<u32>();
    /// # }
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
    /// use futures::future::{self, TryFuture};
    ///
    /// # fn main() {
    /// let future_of_1 = future::ok::<u32, u32>(1);
    /// let future_of_4 = future_of_1.and_then(|x| {
    ///     Ok(x + 3)
    /// });
    ///
    /// let future_of_err_1 = future::err::<u32, u32>(1);
    /// future_of_err_1.and_then(|_| -> TryFuture<u32, u32> {
    ///     panic!("should not be called in case of an error");
    /// });
    /// # }
    /// ```
    fn and_then<B, F>(self, f: F) -> AndThen<Self, B, F>
        where F: FnOnce(Self::Item) -> B,
              B: TryFuture<Error = Self::Error>,
              Self: Sized,
    {
        and_then::new(self, f)
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
    /// use futures::future::{self, TryFuture};
    ///
    /// # fn main() {
    /// let future_of_err_1 = future::err::<u32, u32>(1);
    /// let future_of_4 = future_of_err_1.or_else(|x| -> Result<u32, u32> {
    ///     Ok(x + 3)
    /// });
    ///
    /// let future_of_1 = future::ok::<u32, u32>(1);
    /// future_of_1.or_else(|_| -> TryFuture<u32, u32> {
    ///     panic!("should not be called in case of success");
    /// });
    /// # }
    /// ```
    fn or_else<B, F>(self, f: F) -> OrElse<Self, B, F>
        where F: FnOnce(Self::Error) -> B,
              B: TryFuture<Item = Self::Item>,
              Self: Sized,
    {
        or_else::new(self, f)
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
    /// }
    /// # fn main() {}
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
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let a = future::ok::<u32, u32>(1);
    /// let b = future::ok::<u32, u32>(2);
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
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let a = future::ok::<u32, u32>(1);
    /// let b = future::err::<u32, u32>(2);
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
    /// Because it can never produce an error, the returned `Recover` future can
    /// conform to any specific `Error` type, including `Never`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::err::<(), &str>("something went wrong");
    /// let new_future = future.recover::<Never, _>(|_| ());
    /// assert_eq!(block_on(new_future), Ok(()));
    /// # }
    /// ```
    fn recover<F>(self, f: F) -> Recover<Self, F>
        where Self: Sized,
              F: FnOnce(Self::Error) -> Self::Item
    {
        recover::new(self, f)
    }
}
