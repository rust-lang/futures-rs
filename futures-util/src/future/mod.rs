//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

use core::result;

use futures_core::{Future, IntoFuture, Stream};

// Primitive futures
mod empty;
mod lazy;
mod poll_fn;
mod loop_fn;
pub use self::empty::{empty, Empty};
pub use self::lazy::{lazy, Lazy};
pub use self::poll_fn::{poll_fn, PollFn};
pub use self::loop_fn::{loop_fn, Loop, LoopFn};

// combinators
mod and_then;
mod flatten;
mod flatten_stream;
mod fuse;
mod into_stream;
mod join;
mod map;
mod map_err;
mod err_into;
mod or_else;
mod select;
mod then;
mod inspect;
mod recover;

// impl details
mod chain;

pub use self::and_then::AndThen;
pub use self::flatten::Flatten;
pub use self::flatten_stream::FlattenStream;
pub use self::fuse::Fuse;
pub use self::into_stream::IntoStream;
pub use self::join::{Join, Join3, Join4, Join5};
pub use self::map::Map;
pub use self::map_err::MapErr;
pub use self::err_into::ErrInto;
pub use self::or_else::OrElse;
pub use self::select::Select;
pub use self::then::Then;
pub use self::inspect::Inspect;
pub use self::recover::Recover;

pub use either::Either;

if_std! {
    mod catch_unwind;
    mod join_all;
    mod select_all;
    mod select_ok;
    mod shared;
    pub use self::catch_unwind::CatchUnwind;
    pub use self::join_all::{join_all, JoinAll};
    pub use self::select_all::{SelectAll, SelectAllNext, select_all};
    pub use self::select_ok::{SelectOk, select_ok};
    pub use self::shared::{Shared, SharedItem, SharedError};
}

impl<T: ?Sized> FutureExt for T where T: Future {}

/// An extension trait for `Future`s that provides a variety of convenient
/// combinator functions.
pub trait FutureExt: Future {
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
    fn map<U, F>(self, f: F) -> Map<Self, F>
        where F: FnOnce(Self::Item) -> U,
              Self: Sized,
    {
        assert_future::<U, Self::Error, _>(map::new(self, f))
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
        assert_future::<Self::Item, E, _>(map_err::new(self, f))
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
        assert_future::<Self::Item, E, _>(err_into::new(self))
    }

    /// Chain on a computation for when a future finished, passing the result of
    /// the future to the provided closure `f`.
    ///
    /// This function can be used to ensure a computation runs regardless of
    /// the conclusion of the future. The closure provided will be yielded a
    /// `Result` once the future is complete.
    ///
    /// The returned value of the closure must implement the `IntoFuture` trait
    /// and can represent some more work to be done before the composed future
    /// is finished. Note that the `Result` type implements the `IntoFuture`
    /// trait so it is possible to simply alter the `Result` yielded to the
    /// closure and return it.
    ///
    /// If this future is dropped or panics then the closure `f` will not be
    /// run.
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
    /// let future_of_1 = future::ok::<u32, u32>(1);
    /// let future_of_4 = future_of_1.then(|x| {
    ///     x.map(|y| y + 3)
    /// });
    ///
    /// let future_of_err_1 = future::err::<u32, u32>(1);
    /// let future_of_4 = future_of_err_1.then(|x| {
    ///     match x {
    ///         Ok(_) => panic!("expected an error"),
    ///         Err(y) => future::ok::<u32, u32>(y + 3),
    ///     }
    /// });
    /// # }
    /// ```
    fn then<B, F>(self, f: F) -> Then<Self, B, F>
        where F: FnOnce(result::Result<Self::Item, Self::Error>) -> B,
              B: IntoFuture,
              Self: Sized,
    {
        assert_future::<B::Item, B::Error, _>(then::new(self, f))
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
    /// use futures::future::{self, FutureResult};
    ///
    /// # fn main() {
    /// let future_of_1 = future::ok::<u32, u32>(1);
    /// let future_of_4 = future_of_1.and_then(|x| {
    ///     Ok(x + 3)
    /// });
    ///
    /// let future_of_err_1 = future::err::<u32, u32>(1);
    /// future_of_err_1.and_then(|_| -> FutureResult<u32, u32> {
    ///     panic!("should not be called in case of an error");
    /// });
    /// # }
    /// ```
    fn and_then<B, F>(self, f: F) -> AndThen<Self, B, F>
        where F: FnOnce(Self::Item) -> B,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        assert_future::<B::Item, Self::Error, _>(and_then::new(self, f))
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
    /// use futures::future::{self, FutureResult};
    ///
    /// # fn main() {
    /// let future_of_err_1 = future::err::<u32, u32>(1);
    /// let future_of_4 = future_of_err_1.or_else(|x| -> Result<u32, u32> {
    ///     Ok(x + 3)
    /// });
    ///
    /// let future_of_1 = future::ok::<u32, u32>(1);
    /// future_of_1.or_else(|_| -> FutureResult<u32, u32> {
    ///     panic!("should not be called in case of success");
    /// });
    /// # }
    /// ```
    fn or_else<B, F>(self, f: F) -> OrElse<Self, B, F>
        where F: FnOnce(Self::Error) -> B,
              B: IntoFuture<Item = Self::Item>,
              Self: Sized,
    {
        assert_future::<Self::Item, B::Error, _>(or_else::new(self, f))
    }

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

    /// Wrap this future in an `Either` future, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right` method to write `if`
    /// statements that evaluate to different futures in different branches.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::executor::block_on;
    /// use futures::future::*;
    ///
    /// # fn main() {
    /// let x = 6;
    /// let future = if x < 10 {
    ///     ok::<_, bool>(x).left()
    /// } else {
    ///     empty().right()
    /// };
    ///
    /// assert_eq!(x, block_on(future).unwrap());
    /// # }
    /// ```
    fn left<B>(self) -> Either<Self, B>
        where B: Future<Item = Self::Item, Error = Self::Error>,
              Self: Sized
    {
        Either::Left(self)
    }

    /// Wrap this future in an `Either` future, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left` method to write `if`
    /// statements that evaluate to different futures in different branches.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::executor::block_on;
    /// use futures::future::*;
    ///
    /// # fn main() {
    /// let x = 6;
    /// let future = if x < 10 {
    ///     ok::<_, bool>(x).left()
    /// } else {
    ///     empty().right()
    /// };
    ///
    /// assert_eq!(x, block_on(future).unwrap());
    /// # }
    /// ```
    fn right<A>(self) -> Either<A, Self>
        where A: Future<Item = Self::Item, Error = Self::Error>,
              Self: Sized,
    {
        Either::Right(self)
    }

    /// Convert this future into a single element stream.
    ///
    /// The returned stream contains single success if this future resolves to
    /// success or single error if this future resolves into error.
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
    /// let future = future::ok::<_, bool>(17);
    /// let mut stream = future.into_stream().collect();
    /// assert_eq!(Ok(vec![17]), block_on(stream));
    ///
    /// let future = future::err::<bool, _>(19);
    /// let mut stream = future.into_stream().collect();
    /// assert_eq!(Err(19), block_on(stream));
    /// # }
    /// ```
    fn into_stream(self) -> IntoStream<Self>
        where Self: Sized
    {
        into_stream::new(self)
    }

    /// Flatten the execution of this future when the successful result of this
    /// future is itself another future.
    ///
    /// This can be useful when combining futures together to flatten the
    /// computation out the final result. This method can only be called
    /// when the successful result of this future itself implements the
    /// `IntoFuture` trait and the error can be created from this future's error
    /// type.
    ///
    /// This method is roughly equivalent to `self.and_then(|x| x)`.
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
    /// let nested_future = future::ok::<_, u32>(future::ok::<u32, u32>(1));
    /// let future = nested_future.flatten();
    /// assert_eq!(block_on(future), Ok(1));
    /// # }
    /// ```
    ///
    /// Calling `flatten` on an errored `Future`, or if the inner `Future` is
    /// errored, will result in an errored `Future`:
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let nested_future = future::ok::<_, u32>(future::err::<u32, u32>(1));
    /// let future = nested_future.flatten();
    /// assert_eq!(block_on(future), Err(1));
    /// # }
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Item: IntoFuture<Error = <Self as Future>::Error>,
        Self: Sized
    {
        let f = flatten::new(self);
        assert_future::<<<Self as Future>::Item as IntoFuture>::Item,
                        <<Self as Future>::Item as IntoFuture>::Error,
                        _>(f)
    }

    /// Flatten the execution of this future when the successful result of this
    /// future is a stream.
    ///
    /// This can be useful when stream initialization is deferred, and it is
    /// convenient to work with that stream as if stream was available at the
    /// call site.
    ///
    /// Note that this function consumes this future and returns a wrapped
    /// version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::stream;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let stream_items = vec![17, 18, 19];
    /// let future_of_a_stream = future::ok::<_, bool>(stream::iter_ok(stream_items));
    ///
    /// let stream = future_of_a_stream.flatten_stream();
    /// let list = block_on(stream.collect()).unwrap();
    /// assert_eq!(list, vec![17, 18, 19]);
    /// # }
    /// ```
    fn flatten_stream(self) -> FlattenStream<Self>
        where <Self as Future>::Item: Stream<Error=Self::Error>,
              Self: Sized
    {
        flatten_stream::new(self)
    }

    /// Fuse a future such that `poll` will never again be called once it has
    /// completed.
    ///
    /// Currently once a future has returned `Ready` or `Err` from
    /// `poll` any further calls could exhibit bad behavior such as blocking
    /// forever, panicking, never returning, etc. If it is known that `poll`
    /// may be called too often then this method can be used to ensure that it
    /// has defined semantics.
    ///
    /// Once a future has been `fuse`d and it returns a completion from `poll`,
    /// then it will forever return `Pending` from `poll` again (never
    /// resolve).  This, unlike the trait's `poll` method, is guaranteed.
    ///
    /// This combinator will drop this future as soon as it's been completed to
    /// ensure resources are reclaimed as soon as possible.
    fn fuse(self) -> Fuse<Self>
        where Self: Sized
    {
        let f = fuse::new(self);
        assert_future::<Self::Item, Self::Error, _>(f)
    }

    /// Do something with the item of a future, passing it on.
    ///
    /// When using futures, you'll often chain several of them together.
    /// While working on such code, you might want to check out what's happening at
    /// various parts in the pipeline. To do that, insert a call to inspect().
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
    /// let new_future = future.inspect(|&x| println!("about to resolve: {}", x));
    /// assert_eq!(block_on(new_future), Ok(1));
    /// # }
    /// ```
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
        where F: FnOnce(&Self::Item) -> (),
              Self: Sized,
    {
        assert_future::<Self::Item, Self::Error, _>(inspect::new(self, f))
    }

    /// Catches unwinding panics while polling the future.
    ///
    /// In general, panics within a future can propagate all the way out to the
    /// task level. This combinator makes it possible to halt unwinding within
    /// the future itself. It's most commonly used within task executors. It's
    /// not recommended to use this for error handling.
    ///
    /// Note that this method requires the `UnwindSafe` bound from the standard
    /// library. This isn't always applied automatically, and the standard
    /// library provides an `AssertUnwindSafe` wrapper type to apply it
    /// after-the fact. To assist using this method, the `Future` trait is also
    /// implemented for `AssertUnwindSafe<F>` where `F` implements `Future`.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::future::{self, FutureResult};
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let mut future = future::ok::<i32, u32>(2);
    /// assert!(block_on(future.catch_unwind()).is_ok());
    ///
    /// let mut future = future::lazy(|| -> FutureResult<i32, u32> {
    ///     panic!();
    ///     future::ok::<i32, u32>(2)
    /// });
    /// assert!(block_on(future.catch_unwind()).is_err());
    /// # }
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
        where Self: Sized + ::std::panic::UnwindSafe
    {
        catch_unwind::new(self)
    }

    /// Create a cloneable handle to this future where all handles will resolve
    /// to the same result.
    ///
    /// The shared() method provides a method to convert any future into a
    /// cloneable future. It enables a future to be polled by multiple threads.
    ///
    /// The returned `Shared` future resolves successfully with
    /// `SharedItem<Self::Item>` or erroneously with `SharedError<Self::Error>`.
    /// Both `SharedItem` and `SharedError` implements `Deref` to allow shared
    /// access to the underlying result. Ownership of `Self::Item` and
    /// `Self::Error` cannot currently be reclaimed.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
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
    /// let future = future::ok::<_, bool>(6);
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    ///
    /// assert_eq!(6, *block_on(shared1).unwrap());
    /// assert_eq!(6, *block_on(shared2).unwrap());
    /// # }
    /// ```
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::ok::<_, bool>(6);
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    /// let join_handle = thread::spawn(move || {
    ///     assert_eq!(6, *block_on(shared2).unwrap());
    /// });
    /// assert_eq!(6, *block_on(shared1).unwrap());
    /// join_handle.join().unwrap();
    /// # }
    /// ```
    #[cfg(feature = "std")]
    fn shared(self) -> Shared<Self>
        where Self: Sized
    {
        shared::new(self)
    }

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
    fn recover<E, F>(self, f: F) -> Recover<Self, E, F>
        where Self: Sized,
              F: FnOnce(Self::Error) -> Self::Item
    {
        recover::new(self, f)
    }
}

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
fn assert_future<A, B, F>(t: F) -> F
    where F: Future<Item=A, Error=B>,
{
    t
}
