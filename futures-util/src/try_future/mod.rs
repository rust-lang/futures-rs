//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

use futures_core::future::TryFuture;
use futures_sink::Sink;

#[cfg(feature = "compat")] use crate::compat::Compat;

/* TODO
mod join;
mod select;
pub use self::join::{Join, Join3, Join4, Join5};
pub use self::select::Select;

#[cfg(feature = "std")]
mod join_all;
#[cfg(feature = "std")]
mod select_all;
#[cfg(feature = "std")]
mod select_ok;
#[cfg(feature = "std")]
pub use self::select_all::{SelectAll, SelectAllNext, select_all};
#[cfg(feature = "std")]
pub use self::select_ok::{SelectOk, select_ok};
*/

// Combinators
mod and_then;
pub use self::and_then::AndThen;

mod err_into;
pub use self::err_into::ErrInto;

mod flatten_sink;
pub use self::flatten_sink::FlattenSink;

mod try_join;
pub use self::try_join::{TryJoin, TryJoin3, TryJoin4, TryJoin5};

mod into_future;
pub use self::into_future::IntoFuture;

mod map_err;
pub use self::map_err::MapErr;

mod map_ok;
pub use self::map_ok::MapOk;

mod or_else;
pub use self::or_else::OrElse;

mod unwrap_or_else;
pub use self::unwrap_or_else::UnwrapOrElse;

#[cfg(feature = "std")]
mod join_all;

#[cfg(feature = "std")]
pub use self::join_all::{join_all, JoinAll};

// Implementation details
mod try_chain;
pub(crate) use self::try_chain::{TryChain, TryChainAction};

impl<Fut: TryFuture> TryFutureExt for Fut {}

/// Adapters specific to [`Result`]-returning futures
pub trait TryFutureExt: TryFuture {
    /// Flattens the execution of this future when the successful result of this
    /// future is a [`Sink`].
    ///
    /// This can be useful when sink initialization is deferred, and it is
    /// convenient to work with that sink as if the sink was available at the
    /// call site.
    ///
    /// Note that this function consumes this future and returns a wrapped
    /// version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(futures_api)]
    /// use futures::future::{Future, TryFutureExt};
    /// use futures::sink::Sink;
    /// # use futures::channel::mpsc::{self, SendError};
    /// # type T = i32;
    /// # type E = SendError;
    ///
    /// fn make_sink_async() -> impl Future<Output = Result<
    ///     impl Sink<SinkItem = T, SinkError = E>,
    ///     E,
    /// >> { // ... }
    /// # let (tx, _rx) = mpsc::unbounded::<i32>();
    /// # futures::future::ready(Ok(tx))
    /// # }
    /// fn take_sink(sink: impl Sink<SinkItem = T, SinkError = E>) { /* ... */ }
    ///
    /// let fut = make_sink_async();
    /// take_sink(fut.flatten_sink())
    /// ```
    fn flatten_sink(self) -> FlattenSink<Self, Self::Ok>
    where
        Self::Ok: Sink<SinkError = Self::Error>,
        Self: Sized,
    {
        FlattenSink::new(self)
    }

    /// Maps this future's success value to a different value.
    ///
    /// This method can be used to change the [`Ok`](TryFuture::Ok) type of the
    /// future into a different type. It is similar to the [`Result::map`]
    /// method. You can use this method to chain along a computation once the
    /// future has been resolved.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Ok`]. If it resolves to an [`Err`], panics, or is dropped, then
    /// the provided closure will never be invoked.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Ok::<i32, i32>(1));
    /// let future = future.map_ok(|x| x + 3);
    /// assert_eq!(await!(future), Ok(4));
    /// # });
    /// ```
    ///
    /// Calling [`map_ok`](TryFutureExt::map_ok) on an errored future has no
    /// effect:
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Err::<i32, i32>(1));
    /// let future = future.map_ok(|x| x + 3);
    /// assert_eq!(await!(future), Err(1));
    /// # });
    /// ```
    fn map_ok<T, F>(self, f: F) -> MapOk<Self, F>
        where F: FnOnce(Self::Ok) -> T,
              Self: Sized,
    {
        MapOk::new(self, f)
    }

    /// Maps this future's error value to a different value.
    ///
    /// This method can be used to change the [`Error`](TryFuture::Error) type
    /// of the future into a different type. It is similar to the
    /// [`Result::map_err`] method. You can use this method for example to
    /// ensure that futures have the same [`Error`](TryFuture::Error) type when
    /// using [`select!`] or [`join!`].
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Err`]. If it resolves to an [`Ok`], panics, or is dropped, then
    /// the provided closure will never be invoked.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Err::<i32, i32>(1));
    /// let future = future.map_err(|x| x + 3);
    /// assert_eq!(await!(future), Err(4));
    /// # });
    /// ```
    ///
    /// Calling [`map_err`](TryFutureExt::map_err) on a successful future has
    /// no effect:
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Ok::<i32, i32>(1));
    /// let future = future.map_err(|x| x + 3);
    /// assert_eq!(await!(future), Ok(1));
    /// # });
    /// ```
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E,
              Self: Sized,
    {
        MapErr::new(self, f)
    }

    /// Maps this future's [`Error`](TryFuture::Error) to a new error type
    /// using the [`Into`](std::convert::Into) trait.
    ///
    /// This method does for futures what the `?`-operator does for
    /// [`Result`]: It lets the compiler infer the type of the resulting
    /// error. Just as [`map_err`](TryFutureExt::map_err), this is useful for
    /// example to ensure that futures have the same [`Error`](TryFuture::Error)
    /// type when using [`select!`] or [`join!`].
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future_err_u8 = future::ready(Err::<(), u8>(1));
    /// let future_err_i32 = future_err_u8.err_into::<i32>();
    /// # });
    /// ```
    fn err_into<E>(self) -> ErrInto<Self, E>
        where Self: Sized,
              Self::Error: Into<E>
    {
        ErrInto::new(self)
    }

    /// Executes another future after this one resolves successfully. The
    /// success value is passed to a closure to create this subsequent future.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Ok`]. If this future resolves to an [`Err`], panics, or is
    /// dropped, then the provided closure will never be invoked. The
    /// [`Error`](TryFuture::Error) type of this future and the future
    /// returned by `f` have to match.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Ok::<i32, i32>(1));
    /// let future = future.and_then(|x| future::ready(Ok::<i32, i32>(x + 3)));
    /// assert_eq!(await!(future), Ok(4));
    /// # });
    /// ```
    ///
    /// Calling [`and_then`](TryFutureExt::and_then) on an errored future has no
    /// effect:
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Err::<i32, i32>(1));
    /// let future = future.and_then(|x| future::ready(Err::<i32, i32>(x + 3)));
    /// assert_eq!(await!(future), Err(1));
    /// # });
    /// ```
    fn and_then<Fut, F>(self, f: F) -> AndThen<Self, Fut, F>
        where F: FnOnce(Self::Ok) -> Fut,
              Fut: TryFuture<Error = Self::Error>,
              Self: Sized,
    {
        AndThen::new(self, f)
    }

    /// Executes another future if this one resolves to an error. The
    /// error value is passed to a closure to create this subsequent future.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Err`]. If this future resolves to an [`Ok`], panics, or is
    /// dropped, then the provided closure will never be invoked. The
    /// [`Ok`](TryFuture::Ok) type of this future and the future returned by `f`
    /// have to match.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Err::<i32, i32>(1));
    /// let future = future.or_else(|x| future::ready(Err::<i32, i32>(x + 3)));
    /// assert_eq!(await!(future), Err(4));
    /// # });
    /// ```
    ///
    /// Calling [`or_else`](TryFutureExt::or_else) on a successful future has
    /// no effect:
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Ok::<i32, i32>(1));
    /// let future = future.or_else(|x| future::ready(Ok::<i32, i32>(x + 3)));
    /// assert_eq!(await!(future), Ok(1));
    /// # });
    /// ```
    fn or_else<Fut, F>(self, f: F) -> OrElse<Self, Fut, F>
        where F: FnOnce(Self::Error) -> Fut,
              Fut: TryFuture<Ok = Self::Ok>,
              Self: Sized,
    {
        OrElse::new(self, f)
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
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
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
    /// use futures::executor::block_on;
    /// use futures::future::{self, FutureExt};
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

    /// Unwraps this future's ouput, producing a future with this future's
    /// [`Ok`](TryFuture::Ok) type as its
    /// [`Output`](std::future::Future::Output) type.
    ///
    /// If this future is resolved successfully, the returned future will
    /// contain the original future's success value as output. Otherwise, the
    /// closure `f` is called with the error value to produce an alternate
    /// success value.
    ///
    /// This method is similar to the [`Result::unwrap_or_else`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::future::{self, TryFutureExt};
    ///
    /// # futures::executor::block_on(async {
    /// let future = future::ready(Err::<(), &str>("Boom!"));
    /// let future = future.unwrap_or_else(|_| ());
    /// assert_eq!(await!(future), ());
    /// # });
    /// ```
    fn unwrap_or_else<F>(self, f: F) -> UnwrapOrElse<Self, F>
        where Self: Sized,
              F: FnOnce(Self::Error) -> Self::Ok
    {
        UnwrapOrElse::new(self, f)
    }

    /// Wraps a [`TryFuture`] into a future compatable with libraries using
    /// futures 0.1 future definitons. Requires the `compat` feature to enable.
    #[cfg(feature = "compat")]
    fn compat(self) -> Compat<Self>
        where Self: Sized + Unpin,
    {
        Compat::new(self)
    }

    /// Wraps a [`TryFuture`] into a type that implements
    /// [`Future`](std::future::Future).
    ///
    /// [`TryFuture`]s currently do not implement the
    /// [`Future`](std::future::Future) trait due to limitations of the
    /// compiler.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(futures_api)]
    /// use futures::future::{Future, TryFuture, TryFutureExt};
    ///
    /// # type T = i32;
    /// # type E = ();
    /// fn make_try_future() -> impl TryFuture<Ok = T, Error = E> { // ... }
    /// # futures::future::ready(Ok::<i32, ()>(1))
    /// # }
    /// fn take_future(future: impl Future<Output = Result<T, E>>) { /* ... */ }
    ///
    /// take_future(make_try_future().into_future());
    /// ```
    fn into_future(self) -> IntoFuture<Self>
        where Self: Sized,
    {
        IntoFuture::new(self)
    }

    /// Joins the result of two futures, waiting for them both to complete or
    /// for one to produce an error.
    ///
    /// This function will return a new future which awaits both this and the
    /// `other` future to complete. If successful, the returned future will
    /// finish with a tuple of both results. If unsuccesful, it will complete
    /// with the first error encountered.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// When used on multiple futures that return [`Ok`], `try_join` will return
    /// [`Ok`] of a tuple of the values:
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, TryFutureExt};
    ///
    /// let a = future::ready(Ok::<i32, i32>(1));
    /// let b = future::ready(Ok::<i32, i32>(2));
    /// let pair = a.try_join(b);
    ///
    /// assert_eq!(await!(pair), Ok((1, 2)));
    /// # });
    /// ```
    ///
    /// If one of the futures resolves to an error, `try_join` will return
    /// that error:
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, TryFutureExt};
    ///
    /// let a = future::ready(Ok::<i32, i32>(1));
    /// let b = future::ready(Err::<i32, i32>(2));
    /// let pair = a.try_join(b);
    ///
    /// assert_eq!(await!(pair), Err(2));
    /// # });
    /// ```
    fn try_join<Fut2>(self, other: Fut2) -> TryJoin<Self, Fut2>
    where
        Fut2: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        TryJoin::new(self, other)
    }

    /// Same as [`try_join`](TryFutureExt::try_join), but with more futures.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, TryFutureExt};
    ///
    /// let a = future::ready(Ok::<i32, i32>(1));
    /// let b = future::ready(Ok::<i32, i32>(2));
    /// let c = future::ready(Ok::<i32, i32>(3));
    /// let tuple = a.try_join3(b, c);
    ///
    /// assert_eq!(await!(tuple), Ok((1, 2, 3)));
    /// # });
    /// ```
    fn try_join3<Fut2, Fut3>(
        self,
        future2: Fut2,
        future3: Fut3,
    ) -> TryJoin3<Self, Fut2, Fut3>
    where
        Fut2: TryFuture<Error = Self::Error>,
        Fut3: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        TryJoin3::new(self, future2, future3)
    }

    /// Same as [`try_join`](TryFutureExt::try_join), but with more futures.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, TryFutureExt};
    ///
    /// let a = future::ready(Ok::<i32, i32>(1));
    /// let b = future::ready(Ok::<i32, i32>(2));
    /// let c = future::ready(Ok::<i32, i32>(3));
    /// let d = future::ready(Ok::<i32, i32>(4));
    /// let tuple = a.try_join4(b, c, d);
    ///
    /// assert_eq!(await!(tuple), Ok((1, 2, 3, 4)));
    /// # });
    /// ```
    fn try_join4<Fut2, Fut3, Fut4>(
        self,
        future2: Fut2,
        future3: Fut3,
        future4: Fut4,
    ) -> TryJoin4<Self, Fut2, Fut3, Fut4>
    where
        Fut2: TryFuture<Error = Self::Error>,
        Fut3: TryFuture<Error = Self::Error>,
        Fut4: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        TryJoin4::new(self, future2, future3, future4)
    }

    /// Same as [`try_join`](TryFutureExt::try_join), but with more futures.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, TryFutureExt};
    ///
    /// let a = future::ready(Ok::<i32, i32>(1));
    /// let b = future::ready(Ok::<i32, i32>(2));
    /// let c = future::ready(Ok::<i32, i32>(3));
    /// let d = future::ready(Ok::<i32, i32>(4));
    /// let e = future::ready(Ok::<i32, i32>(5));
    /// let tuple = a.try_join5(b, c, d, e);
    ///
    /// assert_eq!(await!(tuple), Ok((1, 2, 3, 4, 5)));
    /// # });
    /// ```
    fn try_join5<Fut2, Fut3, Fut4, Fut5>(
        self,
        future2: Fut2,
        future3: Fut3,
        future4: Fut4,
        future5: Fut5,
    ) -> TryJoin5<Self, Fut2, Fut3, Fut4, Fut5>
    where
        Fut2: TryFuture<Error = Self::Error>,
        Fut3: TryFuture<Error = Self::Error>,
        Fut4: TryFuture<Error = Self::Error>,
        Fut5: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        TryJoin5::new(self, future2, future3, future4, future5)
    }
}
