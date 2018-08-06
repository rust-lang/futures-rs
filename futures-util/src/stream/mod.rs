//! Streams
//!
//! This module contains a number of functions for working with `Stream`s,
//! including the `StreamExt` trait which adds methods to `Stream` types.

use core::marker::Unpin;
use core::mem::PinMut;
use either::Either;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

mod iter;
pub use self::iter::{iter, Iter};

mod repeat;
pub use self::repeat::{repeat, Repeat};

mod chain;
pub use self::chain::Chain;

mod concat;
pub use self::concat::Concat;

mod empty;
pub use self::empty::{empty, Empty};

mod filter;
pub use self::filter::Filter;

mod filter_map;
pub use self::filter_map::FilterMap;

mod flatten;
pub use self::flatten::Flatten;

mod fold;
pub use self::fold::Fold;

mod forward;
pub use self::forward::Forward;

mod for_each;
pub use self::for_each::ForEach;

mod fuse;
pub use self::fuse::Fuse;

mod into_future;
pub use self::into_future::StreamFuture;

mod inspect;
pub use self::inspect::Inspect;

mod map;
pub use self::map::Map;

mod next;
pub use self::next::Next;

mod once;
pub use self::once::{once, Once};

mod peek;
pub use self::peek::Peekable;

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

mod select;
pub use self::select::Select;

mod skip;
pub use self::skip::Skip;

mod skip_while;
pub use self::skip_while::SkipWhile;

mod take;
pub use self::take::Take;

mod take_while;
pub use self::take_while::TakeWhile;

mod then;
pub use self::then::Then;

mod unfold;
pub use self::unfold::{unfold, Unfold};

mod zip;
pub use self::zip::Zip;

if_std! {
    use std;
    use std::iter::Extend;

    mod buffer_unordered;
    pub use self::buffer_unordered::BufferUnordered;

    mod buffered;
    pub use self::buffered::Buffered;

    mod catch_unwind;
    pub use self::catch_unwind::CatchUnwind;

    mod chunks;
    pub use self::chunks::Chunks;

    mod collect;
    pub use self::collect::Collect;

    mod futures_ordered;
    pub use self::futures_ordered::{futures_ordered, FuturesOrdered};

    mod futures_unordered;
    pub use self::futures_unordered::{futures_unordered, FuturesUnordered};

    mod split;
    pub use self::split::{SplitStream, SplitSink, ReuniteError};

    // ToDo
    // mod select_all;
    // pub use self::select_all::{select_all, SelectAll};
}

impl<T: ?Sized> StreamExt for T where T: Stream {}

/// An extension trait for `Stream`s that provides a variety of convenient
/// combinator functions.
pub trait StreamExt: Stream {
    /// Creates a future that resolves to the next item in the stream.
    ///
    /// Note that because `next` doesn't take ownership over the stream,
    /// the [`Stream`] type must be [`Unpin`]. If you want to use `next` with a
    /// [`!Unpin`](Unpin) stream, you'll first have to pin the stream. This can
    /// be done by wrapping the stream in a [`PinBox`](std::boxed::PinBox) or
    /// pinning it to the stack using the `pin_mut!` macro.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(pin)]
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut stream = stream::iter(1..=3);
    ///
    /// assert_eq!(block_on(stream.next()), Some(1));
    /// assert_eq!(block_on(stream.next()), Some(2));
    /// assert_eq!(block_on(stream.next()), Some(3));
    /// assert_eq!(block_on(stream.next()), None);
    /// ```
    fn next(&mut self) -> Next<'_, Self>
        where Self: Sized + Unpin,
    {
        Next::new(self)
    }

    /// Converts this stream into a future of `(next_item, tail_of_stream)`.
    /// If the stream terminates, then the next item is [`None`].
    ///
    /// The returned future can be used to compose streams and futures together
    /// by placing everything into the "world of futures".
    ///
    /// Note that because `into_future` moves the stream, the [`Stream`] type
    /// must be [`Unpin`]. If you want to use `into_future` with a
    /// [`!Unpin`](Unpin) stream, you'll first have to pin the stream. This can
    /// be done by wrapping the stream in a [`PinBox`](std::boxed::PinBox) or
    /// pinning it to the stack using the `pin_mut!` macro.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(pin)]
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    ///
    /// let (item, stream) = block_on(stream.into_future());
    /// assert_eq!(Some(1), item);
    ///
    /// let (item, stream) = block_on(stream.into_future());
    /// assert_eq!(Some(2), item);
    /// ```
    fn into_future(self) -> StreamFuture<Self>
        where Self: Sized + Unpin,
    {
        StreamFuture::new(self)
    }

    /// Maps this stream's items to a different type, returning a new stream of
    /// the resulting type.
    ///
    /// The provided closure is executed over all elements of this stream as
    /// they are made available. It is executed inline with calls to
    /// [`poll_next`](Stream::poll_next).
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    /// let stream = stream.map(|x| x + 3);
    ///
    /// assert_eq!(vec![4, 5, 6], block_on(stream.collect::<Vec<_>>()));
    /// ```
    fn map<T, F>(self, f: F) -> Map<Self, F>
        where F: FnMut(Self::Item) -> T,
              Self: Sized
    {
        Map::new(self, f)
    }

    /// Filters the values produced by this stream according to the provided
    /// asynchronous predicate.
    ///
    /// As values of this stream are made available, the provided predicate `f`
    /// will be run against them. If the predicate returns a `Future` which
    /// resolves to `true`, then the stream will yield the value, but if the
    /// predicate returns a `Future` which resolves to `false`, then the value
    /// will be discarded and the next value will be produced.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    /// let evens = stream.filter(|x| future::ready(x % 2 == 0));
    ///
    /// assert_eq!(vec![2, 4, 6, 8, 10], block_on(evens.collect::<Vec<_>>()));
    /// ```
    fn filter<Fut, F>(self, f: F) -> Filter<Self, Fut, F>
        where F: FnMut(&Self::Item) -> Fut,
              Fut: Future<Output = bool>,
              Self: Sized,
    {
        Filter::new(self, f)
    }

    /// Filters the values produced by this stream while simultaneously mapping
    /// them to a different type according to the provided asynchronous closure.
    ///
    /// As values of this stream are made available, the provided function will
    /// be run on them. If the future returned by the predicate `f` resolves to
    /// [`Some(item)`](Some) then the stream will yield the value `item`, but if
    /// it resolves to [`None`] then the next value will be produced.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter_map` methods in
    /// the standard library.
    ///
    /// # Examples
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    /// let evens = stream.filter_map(|x| {
    ///     let ret = if x % 2 == 0 { Some(x + 1) } else { None };
    ///     future::ready(ret)
    /// });
    ///
    /// assert_eq!(vec![3, 5, 7, 9, 11], block_on(evens.collect::<Vec<_>>()));
    /// ```
    fn filter_map<Fut, T, F>(self, f: F) -> FilterMap<Self, Fut, F>
        where F: FnMut(Self::Item) -> Fut,
              Fut: Future<Output = Option<T>>,
              Self: Sized,
    {
        FilterMap::new(self, f)
    }

    /// Computes from this stream's items new items of a different type using
    /// an asynchronous closure.
    ///
    /// The provided closure `f` will be called with an `Item` once a value is
    /// ready, it returns a future which will then be run to completion
    /// to produce the next value on this stream.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    /// let stream = stream.then(|x| future::ready(x + 3));
    ///
    /// assert_eq!(vec![4, 5, 6], block_on(stream.collect::<Vec<_>>()));
    /// ```
    fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F>
        where F: FnMut(Self::Item) -> Fut,
              Fut: Future,
              Self: Sized
    {
        Then::new(self, f)
    }

    /// Collect all of the values of this stream into a vector, returning a
    /// future representing the result of that computation.
    ///
    /// The returned future will be resolved when the stream terminates.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::channel::mpsc;
    /// use futures::executor::block_on;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (mut tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (1..=5) {
    ///         tx.unbounded_send(i).unwrap();
    ///     }
    /// });
    ///
    /// let output = block_on(rx.collect::<Vec<i32>>());
    /// assert_eq!(output, vec![1, 2, 3, 4, 5]);
    /// ```
    #[cfg(feature = "std")]
    fn collect<C: Default + Extend<Self::Item>>(self) -> Collect<Self, C>
        where Self: Sized
    {
        Collect::new(self)
    }

    /// Concatenate all items of a stream into a single extendable
    /// destination, returning a future representing the end result.
    ///
    /// This combinator will extend the first item with the contents
    /// of all the subsequent results of the stream. If the stream is
    /// empty, the default value will be returned.
    ///
    /// Works with all collections that implement the
    /// [`Extend`](std::iter::Extend) trait.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::channel::mpsc;
    /// use futures::executor::block_on;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (mut tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (0..3).rev() {
    ///         let n = i * 3;
    ///         tx.unbounded_send(vec![n + 1, n + 2, n + 3]).unwrap();
    ///     }
    /// });
    ///
    /// let result = block_on(rx.concat());
    ///
    /// assert_eq!(result, vec![7, 8, 9, 4, 5, 6, 1, 2, 3]);
    /// ```
    fn concat(self) -> Concat<Self>
    where Self: Sized,
          Self::Item: Extend<<<Self as Stream>::Item as IntoIterator>::Item> +
                      IntoIterator + Default,
    {
        Concat::new(self)
    }

    /// Execute an accumulating asynchronous computation over a stream,
    /// collecting all the values into one final result.
    ///
    /// This combinator will accumulate all values returned by this stream
    /// according to the closure provided. The initial state is also provided to
    /// this method and then is returned again by each execution of the closure.
    /// Once the entire stream has been exhausted the returned future will
    /// resolve to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let number_stream = stream::iter(0..6);
    /// let sum = number_stream.fold(0, |acc, x| future::ready(acc + x));
    /// assert_eq!(block_on(sum), 15);
    /// ```
    fn fold<T, Fut, F>(self, init: T, f: F) -> Fold<Self, Fut, T, F>
        where F: FnMut(T, Self::Item) -> Fut,
              Fut: Future<Output = T>,
              Self: Sized
    {
        Fold::new(self, f, init)
    }

    /// Flattens a stream of streams into just one continuous stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::channel::mpsc;
    /// use futures::executor::block_on;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (tx1, rx1) = mpsc::unbounded();
    /// let (tx2, rx2) = mpsc::unbounded();
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
    /// let output = block_on(rx3.flatten().collect::<Vec<i32>>());
    /// assert_eq!(output, vec![1, 2, 3, 4]);
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Item: Stream,
              Self: Sized
    {
        Flatten::new(self)
    }

    /// Skip elements on this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function, like `Iterator::skip_while`, will skip elements on the
    /// stream until the predicate `f` resolves to `false`. Once one element
    /// returns false all future elements will be returned from the underlying
    /// stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let stream = stream.skip_while(|x| future::ready(*x <= 5));
    ///
    /// assert_eq!(vec![6, 7, 8, 9, 10], block_on(stream.collect::<Vec<_>>()));
    /// ```
    fn skip_while<Fut, F>(self, f: F) -> SkipWhile<Self, Fut, F>
        where F: FnMut(&Self::Item) -> Fut,
              Fut: Future<Output = bool>,
              Self: Sized
    {
        SkipWhile::new(self, f)
    }

    /// Take elements from this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function, like `Iterator::take_while`, will take elements from the
    /// stream until the predicate `f` resolves to `false`. Once one element
    /// returns false it will always return that the stream is done.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let stream = stream.take_while(|x| future::ready(*x <= 5));
    ///
    /// assert_eq!(vec![1, 2, 3, 4, 5], block_on(stream.collect::<Vec<_>>()));
    /// ```
    fn take_while<Fut, F>(self, f: F) -> TakeWhile<Self, Fut, F>
        where F: FnMut(&Self::Item) -> Fut,
              Fut: Future<Output = bool>,
              Self: Sized
    {
        TakeWhile::new(self, f)
    }

    /// Runs this stream to completion, executing the provided asynchronous
    /// closure for each element on the stream.
    ///
    /// The closure provided will be called for each item this stream produces,
    /// yielding a future. That future will then be executed to completion
    /// before moving on to the next item.
    ///
    /// The returned value is a `Future` where the `Output` type is `()`; it is
    /// executed entirely for its side effects.
    ///
    /// To process each item in the stream and produce another stream instead
    /// of a single future, use `then` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut x = 0;
    ///
    /// {
    ///     let fut = stream::repeat(1).take(3).for_each(|item| {
    ///         x += item;
    ///         future::ready(())
    ///     });
    ///     block_on(fut);
    /// }
    ///
    /// assert_eq!(x, 3);
    /// ```
    fn for_each<Fut, F>(self, f: F) -> ForEach<Self, Fut, F>
        where F: FnMut(Self::Item) -> Fut,
              Fut: Future<Output = ()>,
              Self: Sized
    {
        ForEach::new(self, f)
    }

    /// Creates a new stream of at most `n` items of the underlying stream.
    ///
    /// Once `n` items have been yielded from this stream then it will always
    /// return that the stream is done.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10).take(3);
    ///
    /// assert_eq!(vec![1, 2, 3], block_on(stream.collect::<Vec<_>>()));
    /// ```
    fn take(self, n: u64) -> Take<Self>
        where Self: Sized
    {
        Take::new(self, n)
    }

    /// Creates a new stream which skips `n` items of the underlying stream.
    ///
    /// Once `n` items have been skipped from this stream then it will always
    /// return the remaining items on this stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10).skip(5);
    ///
    /// assert_eq!(vec![6, 7, 8, 9, 10], block_on(stream.collect::<Vec<_>>()));
    /// ```
    fn skip(self, n: u64) -> Skip<Self>
        where Self: Sized
    {
        Skip::new(self, n)
    }

    /// Fuse a stream such that [`poll_next`](Stream::poll_next) will never
    /// again be called once it has finished.
    ///
    /// Normally, once a stream has returned [`None`] from
    /// [`poll_next`](Stream::poll_next) any further calls could exhibit bad
    /// behavior such as block forever, panic, never return, etc. If it is known
    /// that [`poll_next`](Stream::poll_next) may be called after stream
    /// has already finished, then this method can be used to ensure that it has
    /// defined semantics.
    ///
    /// The [`poll_next`](Stream::poll_next) method of a `fuse`d stream
    /// is guaranteed to return [`None`] after the underlying stream has
    /// finished.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(futures_api)]
    /// use futures::executor::block_on_stream;
    /// use futures::stream::{self, StreamExt};
    /// use futures::task::Poll;
    ///
    /// let mut x = 0;
    /// let stream = stream::poll_fn(|_| {
    ///     x += 1;
    ///     match x {
    ///         0..=2 => Poll::Ready(Some(x)),
    ///         3 => Poll::Ready(None),
    ///         _ => panic!("should not happen")
    ///     }
    /// }).fuse();
    ///
    /// let mut iter = block_on_stream(stream);
    /// assert_eq!(Some(1), iter.next());
    /// assert_eq!(Some(2), iter.next());
    /// assert_eq!(None, iter.next());
    /// assert_eq!(None, iter.next());
    /// // ...
    /// ```
    fn fuse(self) -> Fuse<Self>
        where Self: Sized
    {
        Fuse::new(self)
    }

    /// Borrows a stream, rather than consuming it.
    ///
    /// This is useful to allow applying stream adaptors while still retaining
    /// ownership of the original stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut stream = stream::iter(1..5);
    ///
    /// let sum = block_on(stream.by_ref()
    ///                          .take(2)
    ///                          .fold(0, |a, b| future::ready(a + b)));
    /// assert_eq!(sum, 3);
    ///
    /// // You can use the stream again
    /// let sum = block_on(stream.take(2).fold(0, |a, b| future::ready(a + b)));
    /// assert_eq!(sum, 7);
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
    /// after-the fact. To assist using this method, the [`Stream`] trait is
    /// also implemented for `AssertUnwindSafe<St>` where `St` implements
    /// [`Stream`].
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(vec![Some(10), None, Some(11)]);
    /// // Panic on second element
    /// let stream_panicking = stream.map(|o| o.unwrap());
    /// // Collect all the results
    /// let stream = stream_panicking.catch_unwind();
    ///
    /// let results: Vec<Result<i32, _>> = block_on(stream.collect());
    /// match results[0] {
    ///     Ok(10) => {}
    ///     _ => panic!("unexpected result!"),
    /// }
    /// assert!(results[1].is_err());
    /// assert_eq!(results.len(), 2);
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
        where Self: Sized + std::panic::UnwindSafe
    {
        CatchUnwind::new(self)
    }

    /// An adaptor for creating a buffered list of pending futures.
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to at most `n` futures and then return the outputs in the
    /// same order as the underlying stream. No more than `n` futures will be
    /// buffered at any point in time, and less than `n` may also be buffered
    /// depending on the state of each future.
    ///
    /// The returned stream will be a stream of each future's output.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "std")]
    fn buffered(self, n: usize) -> Buffered<Self>
        where Self::Item: Future,
              Self: Sized
    {
        Buffered::new(self, n)
    }

    /// An adaptor for creating a buffered list of pending futures (unordered).
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to `n` futures and then return the outputs in the order
    /// in which they complete. No more than `n` futures will be buffered at
    /// any point in time, and less than `n` may also be buffered depending on
    /// the state of each future.
    ///
    /// The returned stream will be a stream of each future's output.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro)]
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let (send_one, recv_one) = oneshot::channel();
    /// let (send_two, recv_two) = oneshot::channel();
    ///
    /// let stream_of_futures = stream::iter(vec![recv_one, recv_two]);
    /// let mut buffered = stream_of_futures.buffer_unordered(10);
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
    #[cfg(feature = "std")]
    fn buffer_unordered(self, n: usize) -> BufferUnordered<Self>
        where Self::Item: Future,
              Self: Sized
    {
        BufferUnordered::new(self, n)
    }

    /// An adapter for zipping two streams together.
    ///
    /// The zipped stream waits for both streams to produce an item, and then
    /// returns that pair. If either stream ends then the zipped stream will
    /// also end.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut stream1 = stream::iter(1..=3);
    /// let mut stream2 = stream::iter(5..=10);
    ///
    /// let vec = block_on(stream1.zip(stream2)
    ///                           .collect::<Vec<_>>());
    /// assert_eq!(vec![(1, 5), (2, 6), (3, 7)], vec);
    /// ```
    ///
    fn zip<St>(self, other: St) -> Zip<Self, St>
        where St: Stream,
              Self: Sized,
    {
        Zip::new(self, other)
    }

    /// Adapter for chaining two stream.
    ///
    /// The resulting stream emits elements from the first stream, and when
    /// first stream reaches the end, emits the elements from the second stream.
    ///
    /// ```
    /// use futures::executor::block_on;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream1 = stream::iter(vec![Ok(10), Err(false)]);
    /// let stream2 = stream::iter(vec![Err(true), Ok(20)]);
    ///
    /// let stream = stream1.chain(stream2);
    ///
    /// let result: Vec<_> = block_on(stream.collect());
    /// assert_eq!(result, vec![
    ///     Ok(10),
    ///     Err(false),
    ///     Err(true),
    ///     Ok(20),
    /// ]);
    /// ```
    fn chain<St>(self, other: St) -> Chain<Self, St>
        where St: Stream<Item = Self::Item>,
              Self: Sized
    {
        Chain::new(self, other)
    }

    /// Creates a new stream which exposes a `peek` method.
    ///
    /// Calling `peek` returns a reference to the next item in the stream.
    fn peekable(self) -> Peekable<Self>
        where Self: Sized
    {
        Peekable::new(self)
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
        Chunks::new(self, capacity)
    }

    /// This combinator will attempt to pull items from both streams. Each
    /// stream will be polled in a round-robin fashion, and whenever a stream is
    /// ready to yield an item that item is yielded.
    ///
    /// After one of the two input stream completes, the remaining one will be
    /// polled exclusively. The returned stream completes when both input
    /// streams have completed.
    ///
    /// Note that this method consumes both streams and returns a wrapped
    /// version of them.
    fn select<St>(self, other: St) -> Select<Self, St>
        where St: Stream<Item = Self::Item>,
              Self: Sized,
    {
        Select::new(self, other)
    }

    /// A future that completes after the given stream has been fully processed
    /// into the sink, including flushing.
    ///
    /// This future will drive the stream to keep producing items until it is
    /// exhausted, sending each item to the sink. It will complete once both the
    /// stream is exhausted and the sink has received and flushed all items.
    /// Note that the sink is **not** closed.
    ///
    /// On completion, the sink is returned.
    ///
    /// Note that this combinator is only usable with `Unpin` sinks.
    /// Sinks that are not `Unpin` will need to be pinned in order to be used
    /// with `forward`.
    fn forward<S>(self, sink: S) -> Forward<Self, S>
    where
        S: Sink + Unpin,
        Self: Stream<Item = Result<S::SinkItem, S::SinkError>> + Sized,
    {
        Forward::new(self, sink)
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
        Inspect::new(self, f)
    }

    /// Wrap this stream in an `Either` stream, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right_stream` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn left_stream<B>(self) -> Either<Self, B>
        where B: Stream<Item = Self::Item>,
              Self: Sized
    {
        Either::Left(self)
    }

    /// Wrap this stream in an `Either` stream, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left_stream` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn right_stream<B>(self) -> Either<B, Self>
        where B: Stream<Item = Self::Item>,
              Self: Sized
    {
        Either::Right(self)
    }

    /// A convenience method for calling [`Stream::poll_next`] on [`Unpin`]
    /// stream types.
    fn poll_next_unpin(
        &mut self,
        cx: &mut task::Context
    ) -> Poll<Option<Self::Item>>
    where Self: Unpin + Sized
    {
        PinMut::new(self).poll_next(cx)
    }
}
