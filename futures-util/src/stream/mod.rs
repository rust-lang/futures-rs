//! Streams
//!
//! This module contains a number of functions for working with `Stream`s,
//! including the `StreamExt` trait which adds methods to `Stream` types.

use futures_core::{IntoFuture, Stream};
use futures_sink::Sink;
use super::future::Either;

mod iter_ok;
pub use self::iter_ok::{iter_ok, IterOk};
mod iter_result;
pub use self::iter_result::{iter_result, IterResult};

mod repeat;
pub use self::repeat::{repeat, Repeat};

mod and_then;
mod chain;
mod concat;
mod empty;
mod filter;
mod filter_map;
mod flatten;
mod fold;
mod for_each;
mod err_into;
mod fuse;
mod future;
mod inspect;
mod inspect_err;
mod map;
mod map_err;
mod once;
mod or_else;
mod peek;
mod poll_fn;
mod select;
mod skip;
mod skip_while;
mod take;
mod take_while;
mod then;
mod unfold;
mod zip;
mod forward;
mod recover;
pub use self::and_then::AndThen;
pub use self::chain::Chain;
pub use self::concat::Concat;
pub use self::empty::{Empty, empty};
pub use self::filter::Filter;
pub use self::filter_map::FilterMap;
pub use self::flatten::Flatten;
pub use self::fold::Fold;
pub use self::for_each::ForEach;
pub use self::err_into::ErrInto;
pub use self::fuse::Fuse;
pub use self::future::StreamFuture;
pub use self::inspect::Inspect;
pub use self::inspect_err::InspectErr;
pub use self::map::Map;
pub use self::map_err::MapErr;
pub use self::once::{Once, once};
pub use self::or_else::OrElse;
pub use self::peek::Peekable;
pub use self::poll_fn::{poll_fn, PollFn};
pub use self::select::Select;
pub use self::skip::Skip;
pub use self::skip_while::SkipWhile;
pub use self::take::Take;
pub use self::take_while::TakeWhile;
pub use self::then::Then;
pub use self::unfold::{Unfold, unfold};
pub use self::zip::Zip;
pub use self::forward::Forward;
pub use self::recover::Recover;

if_std! {
    use std;

    mod buffered;
    mod buffer_unordered;
    mod catch_unwind;
    mod chunks;
    mod collect;
    mod select_all;
    mod split;
    pub mod futures_unordered;
    mod futures_ordered;
    pub use self::buffered::Buffered;
    pub use self::buffer_unordered::BufferUnordered;
    pub use self::catch_unwind::CatchUnwind;
    pub use self::chunks::Chunks;
    pub use self::collect::Collect;
    pub use self::select_all::{select_all, SelectAll};
    pub use self::split::{SplitStream, SplitSink, ReuniteError};
    pub use self::futures_unordered::{futures_unordered, FuturesUnordered};
    pub use self::futures_ordered::{futures_ordered, FuturesOrdered};
}

impl<T: ?Sized> StreamExt for T where T: Stream {}

/// An extension trait for `Stream`s that provides a variety of convenient
/// combinator functions.
pub trait StreamExt: Stream {
    /// Converts this stream into a `Future`.
    ///
    /// A stream can be viewed as a future which will resolve to a pair containing
    /// the next element of the stream plus the remaining stream. If the stream
    /// terminates, then the next element is `None` and the remaining stream is
    /// still passed back, to allow reclamation of its resources.
    ///
    /// The returned future can be used to compose streams and futures together by
    /// placing everything into the "world of futures".
    fn into_future(self) -> StreamFuture<Self>
        where Self: Sized
    {
        future::new(self)
    }

    /// Converts a stream of type `T` to a stream of type `U`.
    ///
    /// The provided closure is executed over all elements of this stream as
    /// they are made available, and the callback will be executed inline with
    /// calls to `poll`.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    ///
    /// # fn main() {
    /// let (_tx, rx) = mpsc::channel::<i32>(1);
    /// let rx = rx.map(|x| x + 3);
    /// # }
    /// ```
    fn map<U, F>(self, f: F) -> Map<Self, F>
        where F: FnMut(Self::Item) -> U,
              Self: Sized
    {
        map::new(self, f)
    }

    /// Converts a stream of error type `T` to a stream of error type `U`.
    ///
    /// The provided closure is executed over all errors of this stream as
    /// they are made available, and the callback will be executed inline with
    /// calls to `poll`.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it, similar to the existing `map_err` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    ///
    /// # fn main() {
    /// let (_tx, rx) = mpsc::channel::<i32>(1);
    /// let rx = rx.map_err(|_| 3);
    /// # }
    /// ```
    fn map_err<U, F>(self, f: F) -> MapErr<Self, F>
        where F: FnMut(Self::Error) -> U,
              Self: Sized
    {
        map_err::new(self, f)
    }

    /// Filters the values produced by this stream according to the provided
    /// predicate.
    ///
    /// As values of this stream are made available, the provided predicate will
    /// be run against them. If the predicate returns a `Future` which resolves
    /// to `true`, then the stream will yield the value, but if the predicate
    /// returns a `Future` which resolves to `false`, then the  value will be
    /// discarded and the next value will be produced.
    ///
    /// All errors are passed through without filtering in this combinator.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it, similar to the existing `filter` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    ///
    /// # fn main() {
    /// let (_tx, rx) = mpsc::channel::<i32>(1);
    /// let evens = rx.filter(|x| Ok(x % 2 == 0));
    /// # }
    /// ```
    fn filter<R, P>(self, pred: P) -> Filter<Self, R, P>
        where P: FnMut(&Self::Item) -> R,
              R: IntoFuture<Item=bool, Error=Self::Error>,
              Self: Sized,
    {
        filter::new(self, pred)
    }

    /// Filters the values produced by this stream while simultaneously mapping
    /// them to a different type.
    ///
    /// As values of this stream are made available, the provided function will
    /// be run on them. If the predicate returns `Some(e)` then the stream will
    /// yield the value `e`, but if the predicate returns `None` then the next
    /// value will be produced.
    ///
    /// All errors are passed through without filtering in this combinator.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it, similar to the existing `filter_map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    ///
    /// # fn main() {
    /// let (_tx, rx) = mpsc::channel::<i32>(1);
    /// let evens_plus_one = rx.filter_map(|x| {
    ///     Ok(
    ///         if x % 0 == 2 {
    ///             Some(x + 1)
    ///         } else {
    ///             None
    ///         }
    ///     )
    /// });
    /// # }
    /// ```
    fn filter_map<R, B, F>(self, f: F) -> FilterMap<Self, R, F>
        where F: FnMut(Self::Item) -> R,
              R: IntoFuture<Item=Option<B>, Error=Self::Error>,
              Self: Sized,
    {
        filter_map::new(self, f)
    }

    /// Chain on a computation for when a value is ready, passing the resulting
    /// item to the provided closure `f`.
    ///
    /// This function can be used to ensure a computation runs regardless of
    /// the next value on the stream. The closure provided will be yielded a
    /// `Result` once a value is ready, and the returned future will then be run
    /// to completion to produce the next value on this stream.
    ///
    /// The returned value of the closure must implement the `IntoFuture` trait
    /// and can represent some more work to be done before the composed stream
    /// is finished. Note that the `Result` type implements the `IntoFuture`
    /// trait so it is possible to simply alter the `Result` yielded to the
    /// closure and return it.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    ///
    /// # fn main() {
    /// let (_tx, rx) = mpsc::channel::<i32>(1);
    ///
    /// let rx = rx.then(|result| {
    ///     match result {
    ///         Ok(e) => Ok(e + 3),
    ///         Err(_) => Err(4),
    ///     }
    /// });
    /// # }
    /// ```
    fn then<U, F>(self, f: F) -> Then<Self, U, F>
        where F: FnMut(Result<Self::Item, Self::Error>) -> U,
              U: IntoFuture,
              Self: Sized
    {
        then::new(self, f)
    }

    /// Chain on a computation for when a value is ready, passing the successful
    /// results to the provided closure `f`.
    ///
    /// This function can be used to run a unit of work when the next successful
    /// value on a stream is ready. The closure provided will be yielded a value
    /// when ready, and the returned future will then be run to completion to
    /// produce the next value on this stream.
    ///
    /// Any errors produced by this stream will not be passed to the closure,
    /// and will be passed through.
    ///
    /// The returned value of the closure must implement the `IntoFuture` trait
    /// and can represent some more work to be done before the composed stream
    /// is finished. Note that the `Result` type implements the `IntoFuture`
    /// trait so it is possible to simply alter the `Result` yielded to the
    /// closure and return it.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it.
    ///
    /// To process the entire stream and return a single future representing
    /// success or error, use `for_each` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    ///
    /// # fn main() {
    /// let (_tx, rx) = mpsc::channel::<i32>(1);
    ///
    /// let rx = rx.and_then(|result| {
    ///     if result % 2 == 0 {
    ///         Ok(Some(result))
    ///     } else {
    ///         Ok(None)
    ///     }
    /// });
    /// # }
    /// ```
    fn and_then<U, F>(self, f: F) -> AndThen<Self, U, F>
        where F: FnMut(Self::Item) -> U,
              U: IntoFuture<Error = Self::Error>,
              Self: Sized
    {
        and_then::new(self, f)
    }

    /// Chain on a computation for when an error happens, passing the
    /// erroneous result to the provided closure `f`.
    ///
    /// This function can be used to run a unit of work and attempt to recover from
    /// an error if one happens. The closure provided will be yielded an error
    /// when one appears, and the returned future will then be run to completion
    /// to produce the next value on this stream.
    ///
    /// Any successful values produced by this stream will not be passed to the
    /// closure, and will be passed through.
    ///
    /// The returned value of the closure must implement the `IntoFuture` trait
    /// and can represent some more work to be done before the composed stream
    /// is finished. Note that the `Result` type implements the `IntoFuture`
    /// trait so it is possible to simply alter the `Result` yielded to the
    /// closure and return it.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it.
    fn or_else<U, F>(self, f: F) -> OrElse<Self, U, F>
        where F: FnMut(Self::Error) -> U,
              U: IntoFuture<Item = Self::Item>,
              Self: Sized
    {
        or_else::new(self, f)
    }

    /// Collect all of the values of this stream into a vector, returning a
    /// future representing the result of that computation.
    ///
    /// This combinator will collect all successful results of this stream and
    /// collect them into a `Vec<Self::Item>`. If an error happens then all
    /// collected elements will be dropped and the error will be returned.
    ///
    /// The returned future will be resolved whenever an error happens or when
    /// the stream returns `Ok(None)`.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// # extern crate futures_channel;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let (mut tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (0..5).rev() {
    ///         tx.unbounded_send(i + 1).unwrap();
    ///     }
    /// });
    ///
    /// let result = block_on(rx.collect());
    /// assert_eq!(result, Ok(vec![5, 4, 3, 2, 1]));
    /// # }
    /// ```
    #[cfg(feature = "std")]
    fn collect(self) -> Collect<Self>
        where Self: Sized
    {
        collect::new(self)
    }

    /// Concatenate all results of a stream into a single extendable
    /// destination, returning a future representing the end result.
    ///
    /// This combinator will extend the first item with the contents
    /// of all the successful results of the stream. If the stream is
    /// empty, the default value will be returned. If an error occurs,
    /// all the results will be dropped and the error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// # extern crate futures_channel;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let (mut tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (0..3).rev() {
    ///         let n = i * 3;
    ///         tx.unbounded_send(vec![n + 1, n + 2, n + 3]).unwrap();
    ///     }
    /// });
    /// let result = block_on(rx.concat());
    /// assert_eq!(result, Ok(vec![7, 8, 9, 4, 5, 6, 1, 2, 3]));
    /// # }
    /// ```
    fn concat(self) -> Concat<Self>
        where Self: Sized,
              Self::Item: Extend<<<Self as Stream>::Item as IntoIterator>::Item> + IntoIterator + Default,
    {
        concat::new(self)
    }

    /// Execute an accumulating computation over a stream, collecting all the
    /// values into one final result.
    ///
    /// This combinator will collect all successful results of this stream
    /// according to the closure provided. The initial state is also provided to
    /// this method and then is returned again by each execution of the closure.
    /// Once the entire stream has been exhausted the returned future will
    /// resolve to this value.
    ///
    /// If an error happens then collected state will be dropped and the error
    /// will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::stream;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let number_stream = stream::iter_ok::<_, ()>(0..6);
    /// let sum = number_stream.fold(0, |acc, x| future::ok(acc + x));
    /// assert_eq!(block_on(sum), Ok(15));
    /// # }
    /// ```
    fn fold<T, Fut, F>(self, init: T, f: F) -> Fold<Self, Fut, T, F>
        where F: FnMut(T, Self::Item) -> Fut,
              Fut: IntoFuture<Item = T, Error = Self::Error>,
              Self: Sized
    {
        fold::new(self, f, init)
    }

    /// Flattens a stream of streams into just one continuous stream.
    ///
    /// If this stream's elements are themselves streams then this combinator
    /// will flatten out the entire stream to one long chain of elements. Any
    /// errors are passed through without looking at them, but otherwise each
    /// individual stream will get exhausted before moving on to the next.
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_channel;
    /// # extern crate futures_executor;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures_channel::mpsc;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let (tx1, rx1) = mpsc::unbounded::<i32>();
    /// let (tx2, rx2) = mpsc::unbounded::<i32>();
    /// let (tx3, rx3) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     tx1.unbounded_send(1).unwrap();
    ///     tx1.unbounded_send(2).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx2.unbounded_send(3).unwrap();
    ///     tx2.unbounded_send(4).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx3.unbounded_send(rx1).unwrap();
    ///     tx3.unbounded_send(rx2).unwrap();
    /// });
    ///
    /// let result = block_on(rx3.flatten().collect());
    /// assert_eq!(result, Ok(vec![1, 2, 3, 4]));
    /// # }
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Item: Stream<Error = Self::Error>,
              Self: Sized
    {
        flatten::new(self)
    }

    /// Skip elements on this stream while the predicate provided resolves to
    /// `true`.
    ///
    /// This function, like `Iterator::skip_while`, will skip elements on the
    /// stream until the `predicate` resolves to `false`. Once one element
    /// returns false all future elements will be returned from the underlying
    /// stream.
    fn skip_while<R, P>(self, pred: P) -> SkipWhile<Self, R, P>
        where P: FnMut(&Self::Item) -> R,
              R: IntoFuture<Item=bool, Error=Self::Error>,
              Self: Sized
    {
        skip_while::new(self, pred)
    }

    /// Take elements from this stream while the predicate provided resolves to
    /// `true`.
    ///
    /// This function, like `Iterator::take_while`, will take elements from the
    /// stream until the `predicate` resolves to `false`. Once one element
    /// returns false it will always return that the stream is done.
    fn take_while<R, P>(self, pred: P) -> TakeWhile<Self, R, P>
        where P: FnMut(&Self::Item) -> R,
              R: IntoFuture<Item=bool, Error=Self::Error>,
              Self: Sized
    {
        take_while::new(self, pred)
    }

    /// Runs this stream to completion, executing the provided closure for each
    /// element on the stream.
    ///
    /// The closure provided will be called for each item this stream resolves
    /// to successfully, producing a future. That future will then be executed
    /// to completion before moving on to the next item.
    ///
    /// The returned value is a `Future` where the `Item` type is `()` and
    /// errors are otherwise threaded through. Any error on the stream or in the
    /// closure will cause iteration to be halted immediately and the future
    /// will resolve to that error.
    ///
    /// To process each item in the stream and produce another stream instead
    /// of a single future, use `and_then` instead.
    fn for_each<U, F>(self, f: F) -> ForEach<Self, U, F>
        where F: FnMut(Self::Item) -> U,
              U: IntoFuture<Item=(), Error = Self::Error>,
              Self: Sized
    {
        for_each::new(self, f)
    }

    /// Map this stream's error to a different type using the `Into` trait.
    ///
    /// This function does for streams what `try!` does for `Result`,
    /// by letting the compiler infer the type of the resulting error.
    /// Just as `map_err` above, this is useful for example to ensure
    /// that streams have the same error type when used with
    /// combinators.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it.
    fn err_into<E>(self) -> ErrInto<Self, E>
        where Self: Sized,
              Self::Error: Into<E>,
    {
        err_into::new(self)
    }

    /// Creates a new stream of at most `amt` items of the underlying stream.
    ///
    /// Once `amt` items have been yielded from this stream then it will always
    /// return that the stream is done.
    ///
    /// # Errors
    ///
    /// Any errors yielded from underlying stream, before the desired amount of
    /// items is reached, are passed through and do not affect the total number
    /// of items taken.
    fn take(self, amt: u64) -> Take<Self>
        where Self: Sized
    {
        take::new(self, amt)
    }

    /// Creates a new stream which skips `amt` items of the underlying stream.
    ///
    /// Once `amt` items have been skipped from this stream then it will always
    /// return the remaining items on this stream.
    ///
    /// # Errors
    ///
    /// All errors yielded from underlying stream are passed through and do not
    /// affect the total number of items skipped.
    fn skip(self, amt: u64) -> Skip<Self>
        where Self: Sized
    {
        skip::new(self, amt)
    }

    /// Fuse a stream such that `poll` will never again be called once it has
    /// finished.
    ///
    /// Currently once a stream has returned `None` from `poll` any further
    /// calls could exhibit bad behavior such as block forever, panic, never
    /// return, etc. If it is known that `poll` may be called after stream has
    /// already finished, then this method can be used to ensure that it has
    /// defined semantics.
    ///
    /// Once a stream has been `fuse`d and it finishes, then it will forever
    /// return `None` from `poll`. This, unlike for the traits `poll` method,
    /// is guaranteed.
    ///
    /// Also note that as soon as this stream returns `None` it will be dropped
    /// to reclaim resources associated with it.
    fn fuse(self) -> Fuse<Self>
        where Self: Sized
    {
        fuse::new(self)
    }

    /// Borrows a stream, rather than consuming it.
    ///
    /// This is useful to allow applying stream adaptors while still retaining
    /// ownership of the original stream.
    ///
    /// ```
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::stream;
    /// use futures::future;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let mut stream = stream::iter_ok::<_, ()>(1..5);
    ///
    /// let sum = block_on(stream.by_ref().take(2).fold(0, |a, b| future::ok(a + b)));
    /// assert_eq!(sum, Ok(3));
    ///
    /// // You can use the stream again
    /// let sum = block_on(stream.take(2).fold(0, |a, b| future::ok(a + b)));
    /// assert_eq!(sum, Ok(7));
    /// # }
    /// ```
    fn by_ref(&mut self) -> &mut Self
        where Self: Sized
    {
        self
    }

    /// Catches unwinding panics while polling the stream.
    ///
    /// Caught panic (if any) will be the last element of the resulting stream.
    ///
    /// In general, panics within a stream can propagate all the way out to the
    /// task level. This combinator makes it possible to halt unwinding within
    /// the stream itself. It's most commonly used within task executors. This
    /// method should not be used for error handling.
    ///
    /// Note that this method requires the `UnwindSafe` bound from the standard
    /// library. This isn't always applied automatically, and the standard
    /// library provides an `AssertUnwindSafe` wrapper type to apply it
    /// after-the fact. To assist using this method, the `Stream` trait is also
    /// implemented for `AssertUnwindSafe<S>` where `S` implements `Stream`.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate futures;
    /// # extern crate futures_executor;
    ///
    /// use futures::prelude::*;
    /// use futures::stream;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let stream = stream::iter_ok::<_, bool>(vec![Some(10), None, Some(11)]);
    /// // panic on second element
    /// let stream_panicking = stream.map(|o| o.unwrap());
    /// // collect all the results
    /// let stream = stream_panicking.catch_unwind().then(|r| Ok::<_, ()>(r));
    ///
    /// let results = block_on(stream.collect()).unwrap();
    /// match results[0] {
    ///     Ok(Ok(10)) => {}
    ///     _ => panic!("unexpected result!"),
    /// }
    /// assert!(results[1].is_err());
    /// assert_eq!(results.len(), 2);
    /// # }
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
        where Self: Sized + std::panic::UnwindSafe
    {
        catch_unwind::new(self)
    }

    /// An adaptor for creating a buffered list of pending futures.
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to at most `amt` futures and then return results in the
    /// same order as the underlying stream. No more than `amt` futures will be
    /// buffered at any point in time, and less than `amt` may also be buffered
    /// depending on the state of each future.
    ///
    /// The returned stream will be a stream of each future's result, with
    /// errors passed through whenever they occur.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "std")]
    fn buffered(self, amt: usize) -> Buffered<Self>
        where Self::Item: IntoFuture<Error = <Self as Stream>::Error>,
              Self: Sized
    {
        buffered::new(self, amt)
    }

    /// An adaptor for creating a buffered list of pending futures (unordered).
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to `amt` futures and then return results in the order
    /// in which they complete. No more than `amt` futures will be buffered at
    /// any point in time, and less than `amt` may also be buffered depending on
    /// the state of each future.
    ///
    /// The returned stream will be a stream of each future's result, with
    /// errors passed through whenever they occur.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "std")]
    fn buffer_unordered(self, amt: usize) -> BufferUnordered<Self>
        where Self::Item: IntoFuture<Error = <Self as Stream>::Error>,
              Self: Sized
    {
        buffer_unordered::new(self, amt)
    }

    /// An adapter for zipping two streams together.
    ///
    /// The zipped stream waits for both streams to produce an item, and then
    /// returns that pair. If an error happens, then that error will be returned
    /// immediately. If either stream ends then the zipped stream will also end.
    fn zip<S>(self, other: S) -> Zip<Self, S>
        where S: Stream<Error = Self::Error>,
              Self: Sized,
    {
        zip::new(self, other)
    }

    /// Adapter for chaining two stream.
    ///
    /// The resulting stream emits elements from the first stream, and when
    /// first stream reaches the end, emits the elements from the second stream.
    ///
    /// ```rust
    /// # extern crate futures;
    /// # extern crate futures_executor;
    /// use futures::prelude::*;
    /// use futures::stream;
    /// use futures_executor::block_on;
    ///
    /// # fn main() {
    /// let stream1 = stream::iter_result(vec![Ok(10), Err(false)]);
    /// let stream2 = stream::iter_result(vec![Err(true), Ok(20)]);
    ///
    /// let stream = stream1.chain(stream2)
    ///     .then(|result| Ok::<_, ()>(result));
    ///
    /// let result = block_on(stream.collect()).unwrap();
    /// assert_eq!(result, vec![
    ///     Ok(10),
    ///     Err(false),
    ///     Err(true),
    ///     Ok(20),
    /// ]);
    /// # }
    /// ```
    fn chain<S>(self, other: S) -> Chain<Self, S>
        where S: Stream<Item = Self::Item, Error = Self::Error>,
              Self: Sized
    {
        chain::new(self, other)
    }

    /// Creates a new stream which exposes a `peek` method.
    ///
    /// Calling `peek` returns a reference to the next item in the stream.
    fn peekable(self) -> Peekable<Self>
        where Self: Sized
    {
        peek::new(self)
    }

    /// An adaptor for chunking up items of the stream inside a vector.
    ///
    /// This combinator will attempt to pull items from this stream and buffer
    /// them into a local vector. At most `capacity` items will get buffered
    /// before they're yielded from the returned stream.
    ///
    /// Note that the vectors returned from this iterator may not always have
    /// `capacity` elements. If the underlying stream ended and only a partial
    /// vector was created, it'll be returned. Additionally if an error happens
    /// from the underlying stream then the currently buffered items will be
    /// yielded.
    ///
    /// Errors are passed through the stream unbuffered.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Panics
    ///
    /// This method will panic of `capacity` is zero.
    #[cfg(feature = "std")]
    fn chunks(self, capacity: usize) -> Chunks<Self>
        where Self: Sized
    {
        chunks::new(self, capacity)
    }

    /// Creates a stream that selects the next element from either this stream
    /// or the provided one, whichever is ready first.
    ///
    /// This combinator will attempt to pull items from both streams. Each
    /// stream will be polled in a round-robin fashion, and whenever a stream is
    /// ready to yield an item that item is yielded.
    ///
    /// The `select` function is similar to `merge` except that it requires both
    /// streams to have the same item and error types.
    ///
    /// Error are passed through from either stream.
    fn select<S>(self, other: S) -> Select<Self, S>
        where S: Stream<Item = Self::Item, Error = Self::Error>,
              Self: Sized,
    {
        select::new(self, other)
    }

    /// A future that completes after the given stream has been fully processed
    /// into the sink, including flushing.
    ///
    /// This future will drive the stream to keep producing items until it is
    /// exhausted, sending each item to the sink. It will complete once both the
    /// stream is exhausted, and the sink has received and flushed all items.
    /// Note that the sink is **not** closed.
    ///
    /// Doing `stream.forward(sink)` is roughly equivalent to
    /// `sink.send_all(stream)`. The returned future will exhaust all items from
    /// `self`, sending them all to `sink`.
    ///
    /// On completion, the pair `(stream, sink)` is returned.
    fn forward<S>(self, sink: S) -> Forward<Self, S>
        where S: Sink<SinkItem = Self::Item>,
              Self::Error: From<S::SinkError>,
              Self: Sized
    {
        forward::new(self, sink)
    }

    /// Splits this `Stream + Sink` object into separate `Stream` and `Sink`
    /// objects.
    ///
    /// This can be useful when you want to split ownership between tasks, or
    /// allow direct interaction between the two objects (e.g. via
    /// `Sink::send_all`).
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "std")]
    fn split(self) -> (SplitSink<Self>, SplitStream<Self>)
        where Self: Sink + Sized
    {
        split::split(self)
    }

    /// Do something with each item of this stream, afterwards passing it on.
    ///
    /// This is similar to the `Iterator::inspect` method in the standard
    /// library where it allows easily inspecting each value as it passes
    /// through the stream, for example to debug what's going on.
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
        where F: FnMut(&Self::Item),
              Self: Sized,
    {
        inspect::new(self, f)
    }

    /// Do something with the error of this stream, afterwards passing it on.
    ///
    /// This is similar to the `Stream::inspect` method where it allows
    /// easily inspecting the error as it passes through the stream, for
    /// example to debug what's going on.
    fn inspect_err<F>(self, f: F) -> InspectErr<Self, F>
        where F: FnMut(&Self::Error),
              Self: Sized,
    {
        inspect_err::new(self, f)
    }

    /// Handle errors generated by this stream by converting them into
    /// `Option<Self::Item>`, such that a `None` value terminates the stream.
    ///
    /// Because it can never produce an error, the returned `Recover` stream can
    /// conform to any specific `Error` type, including `Never`.
    fn recover<E, F>(self, f: F) -> Recover<Self, E, F>
        where F: FnMut(Self::Error) -> Option<Self::Item>,
              Self: Sized,
    {
        recover::new(self, f)
    }


    /// Wrap this stream in an `Either` stream, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn left<B>(self) -> Either<Self, B>
        where B: Stream<Item = Self::Item, Error = Self::Error>,
              Self: Sized
    {
        Either::Left(self)
    }

    /// Wrap this stream in an `Either` stream, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn right<B>(self) -> Either<B, Self>
        where B: Stream<Item = Self::Item, Error = Self::Error>,
              Self: Sized
    {
        Either::Right(self)
    }
}
