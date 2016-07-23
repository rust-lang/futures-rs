//! Asynchronous streams
//!
//! This module contains the `Stream` trait and a number of adaptors for this
//! trait. This trait is very similar to the `Iterator` trait in the standard
//! library except that it expresses the concept of blocking as well. A stream
//! here is a sequential sequence of values which may take some amount of time
//! inbetween to produce.
//!
//! A stream may request that it is blocked between values while the next value
//! is calculated, and provides a way to get notified once the next value is
//! ready as well.
// TODO: expand these docs

use {Task, IntoFuture, Poll};

mod channel;
mod iter;
pub use self::channel::{channel, Sender, Receiver};
pub use self::iter::{iter, IterStream};

mod and_then;
mod buffered;
mod collect;
mod filter;
mod filter_map;
mod flatten;
mod fold;
mod for_each;
mod fuse;
mod future;
mod map;
mod map_err;
mod merge;
mod or_else;
mod skip;
mod skip_while;
mod take;
mod then;
pub use self::and_then::AndThen;
pub use self::buffered::Buffered;
pub use self::collect::Collect;
pub use self::filter::Filter;
pub use self::filter_map::FilterMap;
pub use self::flatten::Flatten;
pub use self::fold::Fold;
pub use self::for_each::ForEach;
pub use self::fuse::Fuse;
pub use self::future::StreamFuture;
pub use self::map::Map;
pub use self::map_err::MapErr;
pub use self::merge::{Merge, MergedItem};
pub use self::or_else::OrElse;
pub use self::skip::Skip;
pub use self::skip_while::SkipWhile;
pub use self::take::Take;
pub use self::then::Then;

mod impls;

/// A stream of values, not all of which have been produced yet.
///
/// `Stream` is a trait to represent any source of sequential events or items
/// which acts like an iterator but may block over time. Like `Future` the
/// methods of `Stream` never block and it is thus suitable for programming in
/// an asynchronous fashion. This trait is very similar to the `Iterator` trait
/// in the standard library where `Some` is used to signal elements of the
/// stream and `None` is used to indicate that the stream is finished.
///
/// Like futures a stream has basic combinators to transform the stream, perform
/// more work on each item, etc.
///
/// # Basic methods
///
/// Like futures, a `Stream` has two core methods which drive processing of data
/// and notifications of when new data might be ready. The `poll` method checks
/// the status of a stream and the `schedule` method is used to receive
/// notifications for when it may be ready to call `poll` again.
///
/// Also like future, a stream has an associated error type to represent that an
/// element of the computation failed for some reason. Errors, however, do not
/// signal the end of the stream.
// TODO: is that last clause correct?
///
/// # Streams as Futures
///
/// Any instance of `Stream` can also be viewed as a `Future` where the resolved
/// value is the next item in the stream along with the rest of the stream. The
/// `into_future` adaptor can be used here to convert any stream into a future
/// for use with other future methods like `join` and `select`.
// TODO: more here
pub trait Stream: Send + 'static {
    /// The type of item this stream will yield on success.
    type Item: Send + 'static;

    /// The type of error this stream may generate.
    type Error: Send + 'static;

    /// Attempt to pull out the next value of this stream, returning `None` if
    /// it's not ready yet.
    ///
    /// This method, like `Future::poll`, is the sole method of pulling out a
    /// value from a stream. The `task` argument is the task of computation that
    /// this stream is running within, and it contains information like
    /// task-local data and tokens of interest.
    ///
    /// Implementors of this trait must ensure that implementations of this
    /// method do not block, as it may cause consumers to behave badly.
    ///
    /// # Return value
    ///
    /// If `Poll::NotReady` is returned then this stream's next value is not
    /// ready yet, and `schedule` can be used to receive a notification for when
    /// the value may become ready in the future. If `Some` is returned then the
    /// returned value represents the next value on the stream. `Err` indicates
    /// an error happened, while `Ok` indicates whether there was a new item on
    /// the stream or whether the stream has terminated.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, that is `Poll::Ok(None)` has been returned,
    /// further calls to `poll` may result in a panic or other "bad behavior".
    /// If this is difficult to guard against then the `fuse` adapter can be
    /// used to ensure that `poll` always has well-defined semantics.
    // TODO: more here
    fn poll(&mut self, task: &mut Task) -> Poll<Option<Self::Item>, Self::Error>;

    // TODO: should there also be a method like `poll` but doesn't return an
    //       item? basically just says "please make more progress internally"
    //       seems crucial for buffering to actually make any sense.

    /// Schedule a task to be notified when this future is ready.
    ///
    /// This is very similar to the `Future::schedule` method which registers
    /// interest. The task provided will only be notified once for the next
    /// value on a stream. If an application is interested in more values on a
    /// stream, then a task needs to be re-scheduled.
    ///
    /// Multiple calls to `schedule` while waiting for one value to be produced
    /// will only result in the final `task` getting notified. Consumers must
    /// take care that if `schedule` is called twice the previous task does not
    /// need to be invoked.
    ///
    /// Implementors of the `Stream` trait are recommended to just blindly pass
    /// around this task rather than manufacture new tasks for contained
    /// futures.
    ///
    /// When the task is notified it will be provided a set of tokens that
    /// represent the set of events which have happened since it was last called
    /// (or the last call to `poll`). These events can later be read during the
    /// `poll` phase to prevent polling too much.
    ///
    /// # Panics
    ///
    /// Once a stream has returned `Ok(None)` (it's been completed) then further
    /// calls to either `poll` or this function, `schedule`, should not be
    /// expected to behave well. A call to `schedule` after a poll has succeeded
    /// may panic, block forever, or otherwise exhibit odd behavior.
    ///
    /// Callers who may call `schedule` after a stream is finished may want to
    /// consider using the `fuse` adaptor which defines the behavior of
    /// `schedule` after a successful poll, but comes with a little bit of
    /// extra cost.
    fn schedule(&mut self, task: &mut Task);

    /// Convenience function for turning this stream into a trait object.
    ///
    /// This simply avoids the need to write `Box::new` and can often help with
    /// type inference as well by always returning a trait object.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel();
    /// let a: Box<Stream<Item=i32, Error=i32>> = rx.boxed();
    /// ```
    fn boxed(self) -> Box<Stream<Item = Self::Item, Error = Self::Error>>
        where Self: Sized
    {
        Box::new(self)
    }

    /// Converts this stream into a `Future`.
    ///
    /// A stream can be viewed as simply a future which will resolve to the next
    /// element of the stream as well as the stream itself. The returned future
    /// can be used to compose streams and futures together by placing
    /// everything into the "world of futures".
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
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    /// let rx = rx.map(|x| x + 3);
    /// ```
    fn map<U, F>(self, f: F) -> Map<Self, F>
        where F: FnMut(Self::Item) -> U + Send + 'static,
              U: Send + 'static,
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
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `map_err` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    /// let rx = rx.map_err(|x| x + 3);
    /// ```
    fn map_err<U, F>(self, f: F) -> MapErr<Self, F>
        where F: FnMut(Self::Error) -> U + Send + 'static,
              U: Send + 'static,
              Self: Sized
    {
        map_err::new(self, f)
    }

    /// Filters the values produced by this stream according to the provided
    /// predicate.
    ///
    /// As values of this stream are made available, the provided predicate will
    /// be run against them. If the predicate returns `true` then the stream
    /// will yield the value, but if the predicate returns `false` then the
    /// value will be discarded and the next value will be produced.
    ///
    /// All errors are passed through without filtering in this combinator.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `filter` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    /// let evens = rx.filter(|x| x % 0 == 2);
    /// ```
    fn filter<F>(self, f: F) -> Filter<Self, F>
        where F: FnMut(&Self::Item) -> bool + Send + 'static,
              Self: Sized
    {
        filter::new(self, f)
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
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `filter_map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    /// let evens_plus_one = rx.filter_map(|x| {
    ///     if x % 0 == 2 {
    ///         Some(x + 1)
    ///     } else {
    ///         None
    ///     }
    /// });
    /// ```
    fn filter_map<F, B>(self, f: F) -> FilterMap<Self, F>
        where F: FnMut(Self::Item) -> Option<B> + Send + 'static,
              Self: Sized
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
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    ///
    /// let rx = rx.then(|result| {
    ///     match result {
    ///         Ok(e) => Ok(e + 3),
    ///         Err(e) => Err(e - 4),
    ///     }
    /// });
    /// ```
    fn then<F, U>(self, f: F) -> Then<Self, F, U>
        where F: FnMut(Result<Self::Item, Self::Error>) -> U + Send + 'static,
              U: IntoFuture,
              Self: Sized
    {
        then::new(self, f)
    }

    /// Chain on a computation for when a value is ready, passing the successful
    /// results to the provided closure `f`.
    ///
    /// This function can be used run a unit of work when the next successful
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
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    ///
    /// let rx = rx.and_then(|result| {
    ///     if result % 2 == 0 {
    ///         Ok(result)
    ///     } else {
    ///         Err(result as u32)
    ///     }
    /// });
    /// ```
    fn and_then<F, U>(self, f: F) -> AndThen<Self, F, U>
        where F: FnMut(Self::Item) -> U + Send + 'static,
              U: IntoFuture<Error = Self::Error>,
              Self: Sized
    {
        and_then::new(self, f)
    }

    /// Chain on a computation for when an error happens, passing the
    /// erroneous result to the provided closure `f`.
    ///
    /// This function can be used run a unit of work and attempt to recover from
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
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::*;
    ///
    /// let (_tx, rx) = channel::<i32, u32>();
    ///
    /// let rx = rx.or_else(|result| {
    ///     if result % 2 == 0 {
    ///         Ok(result as i32)
    ///     } else {
    ///         Err(result)
    ///     }
    /// });
    /// ```
    fn or_else<F, U>(self, f: F) -> OrElse<Self, F, U>
        where F: FnMut(Self::Error) -> U + Send + 'static,
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
    /// # Examples
    ///
    /// ```
    /// use futures::{finished, Future, Task, Poll};
    /// use futures::stream::*;
    ///
    /// let (tx, rx) = channel::<i32, u32>();
    ///
    /// fn send(n: i32, tx: Sender<i32, u32>)
    ///         -> Box<Future<Item=(), Error=()>> {
    ///     if n == 0 {
    ///         return finished(()).boxed()
    ///     }
    ///     tx.send(Ok(n)).map_err(|_| ()).and_then(move |tx| {
    ///         send(n - 1, tx)
    ///     }).boxed()
    /// }
    ///
    /// send(5, tx).forget();
    ///
    /// let mut result = rx.collect();
    /// assert_eq!(result.poll(&mut Task::new()),
    ///            Poll::Ok(vec![5, 4, 3, 2, 1]));
    /// ```
    fn collect(self) -> Collect<Self>
        where Self: Sized
    {
        collect::new(self)
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
    /// use futures::{finished, Future, Task, Poll};
    /// use futures::stream::*;
    ///
    /// let (tx, rx) = channel::<i32, u32>();
    ///
    /// fn send(n: i32, tx: Sender<i32, u32>)
    ///         -> Box<Future<Item=(), Error=()>> {
    ///     if n == 0 {
    ///         return finished(()).boxed()
    ///     }
    ///     tx.send(Ok(n)).map_err(|_| ()).and_then(move |tx| {
    ///         send(n - 1, tx)
    ///     }).boxed()
    /// }
    ///
    /// send(5, tx).forget();
    ///
    /// let mut result = rx.fold(0, |a, b| finished::<i32, u32>(a + b));
    /// assert_eq!(result.poll(&mut Task::new()), Poll::Ok(15));
    /// ```
    fn fold<F, T, Fut>(self, init: T, f: F) -> Fold<Self, F, Fut, T>
        where F: FnMut(T, Self::Item) -> Fut + Send + 'static,
              Fut: IntoFuture<Item = T>,
              Fut::Error: Into<Self::Error>,
              T: Send + 'static,
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
    /// use futures::{finished, Future, Task, Poll};
    /// use futures::stream::*;
    ///
    /// let (tx1, rx1) = channel::<i32, u32>();
    /// let (tx2, rx2) = channel::<i32, u32>();
    /// let (tx3, rx3) = channel::<_, u32>();
    ///
    /// tx1.send(Ok(1)).and_then(|tx1| tx1.send(Ok(2))).forget();
    /// tx2.send(Ok(3)).and_then(|tx2| tx2.send(Ok(4))).forget();
    ///
    /// tx3.send(Ok(rx1)).and_then(|tx3| tx3.send(Ok(rx2))).forget();
    ///
    /// let mut result = rx3.flatten().collect();
    /// assert_eq!(result.poll(&mut Task::new()), Poll::Ok(vec![1, 2, 3, 4]));
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Item: Stream,
              <Self::Item as Stream>::Error: From<Self::Error>,
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
    fn skip_while<P, R>(self, pred: P) -> SkipWhile<Self, P, R>
        where P: FnMut(&Self::Item) -> R + Send + 'static,
              R: IntoFuture<Item=bool, Error=Self::Error>,
              Self: Sized
    {
        skip_while::new(self, pred)
    }

    // TODO: should this closure return a result?
    #[allow(missing_docs)]
    fn for_each<F>(self, f: F) -> ForEach<Self, F>
        where F: FnMut(Self::Item) -> Result<(), Self::Error> + Send + 'static,
              Self: Sized
    {
        for_each::new(self, f)
    }

    /// Creates a new stream of at most `amt` items.
    ///
    /// Once `amt` items have been yielded from this stream then it will always
    /// return that the stream is done.
    fn take(self, amt: u64) -> Take<Self>
        where Self: Sized
    {
        take::new(self, amt)
    }

    /// Creates a new stream which skips `amt` items of the underlying stream.
    ///
    /// Once `amt` items have been skipped from this stream then it will always
    /// return the remaining items on this stream.
    fn skip(self, amt: u64) -> Skip<Self>
        where Self: Sized
    {
        skip::new(self, amt)
    }

    /// Fuse a stream such that `poll`/`schedule` will never again be called
    /// once it has terminated (signaled emptyness or an error).
    ///
    /// Currently once a stream has returned `Some(Ok(None))` from `poll` any further
    /// calls could exhibit bad behavior such as block forever, panic, never
    /// return, etc. If it is known that `poll` may be called too often then
    /// this method can be used to ensure that it has defined semantics.
    ///
    /// Once a stream has been `fuse`d and it terminates, then
    /// it will forever return `None` from `poll` again (never resolve). This,
    /// unlike the trait's `poll` method, is guaranteed.
    ///
    /// Additionally, once a stream has completed, this `Fuse` combinator will
    /// never call `schedule` on the underlying stream.
    fn fuse(self) -> Fuse<Self>
        where Self: Sized
    {
        fuse::new(self)
    }

    /// An adaptor for creating a buffered list of pending futures.
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to `amt` futures and then return results in the order
    /// that the futures are completed. No more than `amt` futures will be
    /// buffered at any point in time, and less than `amt` may also be buffered
    /// depending on the state of each future.
    ///
    /// The returned stream will be a stream of each future's result, with
    /// errors passed through whenever they occur.
    fn buffered(self, amt: usize) -> Buffered<Self>
        where Self::Item: IntoFuture<Error = <Self as Stream>::Error>,
              Self: Sized
    {
        buffered::new(self, amt)
    }

    /// An adapter for merging the output of two streams.
    ///
    /// The merged stream produces items from one or both of the underlying
    /// streams as they become available. Errors, however, are not merged: you
    /// get at most one error at a time.
    fn merge<S>(self, other: S) -> Merge<Self, S>
        where S: Stream<Error = Self::Error>,
              Self: Sized,
    {
        merge::new(self, other)
    }
}
