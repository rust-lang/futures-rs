//! Streams
//!
//! This module contains a number of functions for working with `Streams`s
//! that return `Result`s, allowing for short-circuiting computations.

use core::marker::Unpin;
use futures_core::future::TryFuture;
use futures_core::stream::TryStream;

#[cfg(feature = "compat")]
use crate::compat::Compat;

#[cfg(feature = "compat")]
use futures_core::task::Executor;

mod err_into;
pub use self::err_into::ErrInto;

mod into_stream;
pub use self::into_stream::IntoStream;

mod map_ok;
pub use self::map_ok::MapOk;

mod map_err;
pub use self::map_err::MapErr;

mod try_next;
pub use self::try_next::TryNext;

mod try_for_each;
pub use self::try_for_each::TryForEach;

mod try_filter_map;
pub use self::try_filter_map::TryFilterMap;

mod try_fold;
pub use self::try_fold::TryFold;

mod try_skip_while;
pub use self::try_skip_while::TrySkipWhile;

if_std! {
    mod try_buffer_unordered;
    pub use self::try_buffer_unordered::TryBufferUnordered;

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
    /// use futures::stream::{self, TryStreamExt};
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
    /// use futures::stream::{self, TryStreamExt};
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
    /// use futures::stream::{self, TryStreamExt};
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

    /// Wraps a [`TryStream`] into a type that implements
    /// [`Stream`](futures_core::Stream)
    ///
    /// [`TryStream`]s currently do not implement the
    /// [`Stream`](futures_core::Stream) trait because of limitations
    /// of the compiler.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::{Stream, TryStream, TryStreamExt};
    ///
    /// # type T = i32;
    /// # type E = ();
    /// fn make_try_stream() -> impl TryStream<Ok = T, Error = E> { // ... }
    /// # futures::stream::empty()
    /// # }
    /// fn take_stream(stream: impl Stream<Item = Result<T, E>>) { /* ... */ }
    ///
    /// take_stream(make_try_stream().into_stream());
    /// ```
    fn into_stream(self) -> IntoStream<Self>
        where Self: Sized,
    {
        IntoStream::new(self)
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
    /// use futures::stream::{self, TryStreamExt};
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
    /// use futures::future;
    /// use futures::stream::{self, TryStreamExt};
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

    /// Skip elements on this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function is similar to [`StreamExt::skip_while`](crate::stream::StreamExt::skip_while)
    /// but exits early if an error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(3), Ok(2)]);
    /// let mut stream = stream.try_skip_while(|x| future::ready(Ok(*x < 3)));
    ///
    /// let output: Result<Vec<i32>, i32> = await!(stream.try_collect());
    /// assert_eq!(output, Ok(vec![3, 2]));
    /// # })
    /// ```
    fn try_skip_while<Fut, F>(self, f: F) -> TrySkipWhile<Self, Fut, F>
        where F: FnMut(&Self::Ok) -> Fut,
              Fut: TryFuture<Ok = bool, Error = Self::Error>,
              Self: Sized
    {
        TrySkipWhile::new(self, f)
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
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::executor::block_on;
    /// use futures::stream::TryStreamExt;
    /// use std::thread;
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
    /// let output: Result<Vec<i32>, i32> = await!(rx.try_collect());
    /// assert_eq!(output, Err(6));
    /// # })
    /// ```
    #[cfg(feature = "std")]
    fn try_collect<C: Default + Extend<Self::Ok>>(self) -> TryCollect<Self, C>
        where Self: Sized
    {
        TryCollect::new(self)
    }

    /// Attempt to filter the values produced by this stream while
    /// simultaneously mapping them to a different type according to the
    /// provided asynchronous closure.
    ///
    /// As values of this stream are made available, the provided function will
    /// be run on them. If the future returned by the predicate `f` resolves to
    /// [`Some(item)`](Some) then the stream will yield the value `item`, but if
    /// it resolves to [`None`] then the next value will be produced.
    ///
    /// All errors are passed through without filtering in this combinator.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter_map` methods in
    /// the standard library.
    ///
    /// # Examples
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok(1i32), Ok(6i32), Err("error")]);
    /// let mut halves = stream.try_filter_map(|x| {
    ///     let ret = if x % 2 == 0 { Some(x / 2) } else { None };
    ///     future::ready(Ok(ret))
    /// });
    ///
    /// assert_eq!(await!(halves.next()), Some(Ok(3)));
    /// assert_eq!(await!(halves.next()), Some(Err("error")));
    /// # })
    /// ```
    fn try_filter_map<Fut, F, T>(self, f: F) -> TryFilterMap<Self, Fut, F>
        where Fut: TryFuture<Ok = Option<T>, Error = Self::Error>,
              F: FnMut(Self::Ok) -> Fut,
              Self: Sized
    {
        TryFilterMap::new(self, f)
    }


    /// Attempt to execute an accumulating asynchronous computation over a
    /// stream, collecting all the values into one final result.
    ///
    /// This combinator will accumulate all values returned by this stream
    /// according to the closure provided. The initial state is also provided to
    /// this method and then is returned again by each execution of the closure.
    /// Once the entire stream has been exhausted the returned future will
    /// resolve to this value.
    ///
    /// This method is similar to [`fold`](super::StreamExt::fold), but will
    /// exit early if an error is encountered in either the stream or the
    /// provided closure.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let number_stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(2)]);
    /// let sum = number_stream.try_fold(0, |acc, x| future::ready(Ok(acc + x)));
    /// assert_eq!(await!(sum), Ok(3));
    ///
    /// let number_stream_with_err = stream::iter(vec![Ok::<i32, i32>(1), Err(2), Ok(1)]);
    /// let sum = number_stream_with_err.try_fold(0, |acc, x| future::ready(Ok(acc + x)));
    /// assert_eq!(await!(sum), Err(2));
    /// # })
    /// ```
    fn try_fold<T, Fut, F>(self, init: T, f: F) -> TryFold<Self, Fut, T, F>
        where F: FnMut(T, Self::Ok) -> Fut,
              Fut: TryFuture<Ok = T, Error = Self::Error>,
              Self: Sized,
    {
        TryFold::new(self, f, init)
    }

    /// Attempt to execute several futures from a stream concurrently.
    ///
    /// This stream's `Ok` type must be a [`TryFuture`] with an `Error` type
    /// that matches the stream's `Error` type.
    ///
    /// This adaptor will buffer up to `n` futures and then return their
    /// outputs in the order in which they complete. If the underlying stream
    /// returns an error, it will be immediately propagated.
    ///
    /// The returned stream will be a stream of results, each containing either
    /// an error or a future's output. An error can be produced either by the
    /// underlying stream itself or by one of the futures it yielded.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// Results are returned in the order of completion:
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    ///
    /// let (send_one, recv_one) = oneshot::channel();
    /// let (send_two, recv_two) = oneshot::channel();
    ///
    /// let stream_of_futures = stream::iter(vec![Ok(recv_one), Ok(recv_two)]);
    ///
    /// let mut buffered = stream_of_futures.try_buffer_unordered(10);
    ///
    /// send_two.send(2i32);
    /// assert_eq!(await!(buffered.next()), Some(Ok(2i32)));
    ///
    /// send_one.send(1i32);
    /// assert_eq!(await!(buffered.next()), Some(Ok(1i32)));
    ///
    /// assert_eq!(await!(buffered.next()), None);
    /// # })
    /// ```
    ///
    /// Errors from the underlying stream itself are propagated:
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::future;
    /// use futures::stream::{StreamExt, TryStreamExt};
    ///
    /// let (sink, stream_of_futures) = mpsc::unbounded();
    /// let mut buffered = stream_of_futures.try_buffer_unordered(10);
    ///
    /// sink.unbounded_send(Ok(future::ready(Ok(7i32))));
    /// assert_eq!(await!(buffered.next()), Some(Ok(7i32)));
    ///
    /// sink.unbounded_send(Err("error in the stream"));
    /// assert_eq!(await!(buffered.next()), Some(Err("error in the stream")));
    /// # })
    /// ```
    #[cfg(feature = "std")]
    fn try_buffer_unordered(self, n: usize) -> TryBufferUnordered<Self>
        where Self::Ok: TryFuture<Error = Self::Error>,
              Self: Sized
    {
        TryBufferUnordered::new(self, n)
    }

    /// Wraps a [`TryStream`] into a stream compatible with libraries using
    /// futures 0.1 `Stream`. Requires the `compat` feature to be enabled.
    #[cfg(feature = "compat")]
    fn compat<E>(self, executor: E) -> Compat<Self, E>
        where Self: Sized + Unpin,
              E: Executor,
    {
        Compat::new(self, Some(executor))
    }
}
