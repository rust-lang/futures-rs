//! Streams
//!
//! This module contains a number of functions for working with `Streams`s
//! that return `Result`s, allowing for short-circuiting computations.

use core::marker::Unpin;
use futures_core::future::TryFuture;
use futures_core::stream::TryStream;

mod err_into;
pub use self::err_into::ErrInto;

mod map_ok;
pub use self::map_ok::MapOk;

mod map_err;
pub use self::map_err::MapErr;

mod try_next;
pub use self::try_next::TryNext;

mod try_for_each;
pub use self::try_for_each::TryForEach;

if_std! {
    mod try_collect;
    pub use self::try_collect::TryCollect;
}

impl<S: TryStream> TryStreamExt for S {}

/// Adapters specific to `Result`-returning streams
pub trait TryStreamExt: TryStream {
    /// Wraps the current stream in a new stream which converts the error type
    /// into the one provided.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::{stream, TryStreamExt};
    ///
    /// let mut stream =
    ///     stream::iter(vec![Ok(()), Err(5i32)])
    ///         .err_into::<i64>();
    ///
    /// assert_eq!(await!(stream.try_next()), Ok(Some(())));
    /// assert_eq!(await!(stream.try_next()), Err(5i64));
    /// # })
    /// ```
    fn err_into<E>(self) -> ErrInto<Self, E>
    where
        Self: Sized,
        Self::Error: Into<E>
    {
        ErrInto::new(self)
    }

    /// Wraps the current stream in a new stream which maps the success value
    /// using the provided closure.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::{stream, TryStreamExt};
    ///
    /// let mut stream =
    ///     stream::iter(vec![Ok(5), Err(0)])
    ///         .map_ok(|x| x + 2);
    ///
    /// assert_eq!(await!(stream.try_next()), Ok(Some(7)));
    /// assert_eq!(await!(stream.try_next()), Err(0));
    /// # })
    /// ```
    fn map_ok<T, F>(self, f: F) -> MapOk<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Ok) -> T,
    {
        MapOk::new(self, f)
    }

    /// Wraps the current stream in a new stream which maps the error value
    /// using the provided closure.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::{stream, TryStreamExt};
    ///
    /// let mut stream =
    ///     stream::iter(vec![Ok(5), Err(0)])
    ///         .map_err(|x| x + 2);
    ///
    /// assert_eq!(await!(stream.try_next()), Ok(Some(5)));
    /// assert_eq!(await!(stream.try_next()), Err(2));
    /// # })
    /// ```
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Error) -> E,
    {
        MapErr::new(self, f)
    }

    /// Creates a future that attempts to resolve the next item in the stream.
    /// If an error is encountered before the next item, the error is returned
    /// instead.
    ///
    /// This is similar to the `Stream::next` combinator, but returns a
    /// `Result<Option<T>, E>` rather than an `Option<Result<T, E>>`, making
    /// for easy use with the `?` operator.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::{stream, TryStreamExt};
    ///
    /// let mut stream = stream::iter(vec![Ok(()), Err(())]);
    ///
    /// assert_eq!(await!(stream.try_next()), Ok(Some(())));
    /// assert_eq!(await!(stream.try_next()), Err(()));
    /// # })
    /// ```
    fn try_next(&mut self) -> TryNext<'_, Self>
        where Self: Sized + Unpin,
    {
        TryNext::new(self)
    }

    /// Attempts to run this stream to completion, executing the provided
    /// asynchronous closure for each element on the stream.
    ///
    /// The provided closure will be called for each item this stream produces,
    /// yielding a future. That future will then be executed to completion
    /// before moving on to the next item.
    ///
    /// The returned value is a [`Future`](futures_core::Future) where the
    /// [`Output`](futures_core::Future::Output) type is
    /// `Result<(), Self::Error>`. If any of the intermediate
    /// futures or the stream returns an error, this future will return
    /// immediately with an error.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::{future, stream, TryStreamExt};
    ///
    /// let mut x = 0i32;
    ///
    /// {
    ///     let fut = stream::repeat(Ok(1)).try_for_each(|item| {
    ///         x += item;
    ///         future::ready(if x == 3 { Err(()) } else { Ok(()) })
    ///     });
    ///     assert_eq!(await!(fut), Err(()));
    /// }
    ///
    /// assert_eq!(x, 3);
    /// # })
    /// ```
    fn try_for_each<Fut, F>(self, f: F) -> TryForEach<Self, Fut, F>
        where F: FnMut(Self::Ok) -> Fut,
              Fut: TryFuture<Ok = (), Error=Self::Error>,
              Self: Sized
    {
        TryForEach::new(self, f)
    }

    /// Attempt to Collect all of the values of this stream into a vector,
    /// returning a future representing the result of that computation.
    ///
    /// This combinator will collect all successful results of this stream and
    /// collect them into a `Vec<Self::Item>`. If an error happens then all
    /// collected elements will be dropped and the error will be returned.
    ///
    /// The returned future will be resolved when the stream terminates.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures::channel::mpsc;
    /// use futures::executor::block_on;
    ///
    /// let (mut tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (1..=5) {
    ///         tx.unbounded_send(Ok(i)).unwrap();
    ///     }
    ///     tx.unbounded_send(Err(6)).unwrap();
    /// });
    ///
    /// let output: Result<Vec<i32>, i32> = block_on(rx.try_collect());
    /// assert_eq!(output, Err(6));
    /// ```
    #[cfg(feature = "std")]
    fn try_collect<C: Default + Extend<Self::Ok>>(self) -> TryCollect<Self, C>
        where Self: Sized
    {
        TryCollect::new(self)
    }
}
