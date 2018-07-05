//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

use futures_core::{Future, Stream};

// Primitive futures
mod empty;
pub use self::empty::{empty, Empty};

mod lazy;
pub use self::lazy::{lazy, Lazy};

mod maybe_done;
pub use self::maybe_done::{maybe_done, MaybeDone};

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

// Combinators
mod flatten;
pub use self::flatten::Flatten;

mod flatten_stream;
pub use self::flatten_stream::FlattenStream;

mod fuse;
pub use self::fuse::Fuse;

mod into_stream;
pub use self::into_stream::IntoStream;

mod join;
pub use self::join::{Join, Join3, Join4, Join5};

mod map;
pub use self::map::Map;

// Todo
// mod select;
// pub use self::select::Select;

mod then;
pub use self::then::Then;

mod inspect;
pub use self::inspect::Inspect;

// Implementation details
mod chain;
crate use self::chain::Chain;

if_std! {
    mod catch_unwind;
    pub use self::catch_unwind::CatchUnwind;

    // ToDo
    // mod join_all;
    // pub use self::join_all::{join_all, JoinAll};

    // mod select_all;
    // pub use self::select_all::{SelectAll, SelectAllNext, select_all};

    // mod select_ok;
    // pub use self::select_ok::{SelectOk, select_ok};

    mod shared;
    pub use self::shared::Shared;

    mod with_executor;
    pub use self::with_executor::WithExecutor;
}

impl<T: ?Sized> FutureExt for T where T: Future {}

/// An extension trait for `Future`s that provides a variety of convenient
/// adapters.
pub trait FutureExt: Future {
    /// Map this future's result to a different type, returning a new future of
    /// the resulting type.
    ///
    /// This function is similar to the `Option::map` or `Iterator::map` where
    /// it will change the type of the underlying future. This is useful to
    /// chain along a computation once a future has been resolved.
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
    /// # fn main() {
    /// let future = future::ready(1);
    /// let new_future = future.map(|x| x + 3);
    /// assert_eq!(block_on(new_future), 4);
    /// # }
    /// ```
    fn map<U, F>(self, f: F) -> Map<Self, F>
        where F: FnOnce(Self::Output) -> U,
              Self: Sized,
    {
        assert_future::<U, _>(Map::new(self, f))
    }

    /// Chain on a computation for when a future finished, passing the result of
    /// the future to the provided closure `f`.
    ///
    /// The returned value of the closure must implement the `Future` trait
    /// and can represent some more work to be done before the composed future
    /// is finished.
    ///
    /// The closure `f` is only run *after* successful completion of the `self`
    /// future.
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
    /// let future_of_1 = future::ready(1);
    /// let future_of_4 = future_of_1.then(|x| future::ready(x + 3));
    /// assert_eq!(block_on(future_of_4), 4);
    /// ```
    fn then<Fut, F>(self, async_op: F) -> Then<Self, Fut, F>
        where F: FnOnce(Self::Output) -> Fut,
              Fut: Future,
              Self: Sized,
    {
        assert_future::<Fut::Output, _>(Then::new(self, async_op))
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
    */

    /// Joins the result of two futures, waiting for them both to complete.
    ///
    /// This function will return a new future which awaits both this and the
    /// `other` future to complete. The returned future will finish with a tuple
    /// of both results.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let a = future::ready(1);
    /// let b = future::ready(2);
    /// let pair = a.join(b);
    ///
    /// assert_eq!(block_on(pair), (1, 2));
    /// # }
    /// ```
    fn join<B>(self, other: B) -> Join<Self, B>
    where
        B: Future,
        Self: Sized,
    {
        let f = join::new(self, other);
        assert_future::<(Self::Output, B::Output), _>(f)
    }

    /// Same as `join`, but with more futures.
    fn join3<B, C>(self, b: B, c: C) -> Join3<Self, B, C>
    where
        B: Future,
        C: Future,
        Self: Sized,
    {
        join::new3(self, b, c)
    }

    /// Same as `join`, but with more futures.
    fn join4<B, C, D>(self, b: B, c: C, d: D) -> Join4<Self, B, C, D>
    where
        B: Future,
        C: Future,
        D: Future,
        Self: Sized,
    {
        join::new4(self, b, c, d)
    }

    /// Same as `join`, but with more futures.
    fn join5<B, C, D, E>(self, b: B, c: C, d: D, e: E) -> Join5<Self, B, C, D, E>
    where
        B: Future,
        C: Future,
        D: Future,
        E: Future,
        Self: Sized,
    {
        join::new5(self, b, c, d, e)
    }

    /* ToDo: futures-core cannot implement Future for Either anymore because of
             the orphan rule. Remove? Implement our own `Either`?
    /// Wrap this future in an `Either` future, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right_future` method to write `if`
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
    ///     ready(true).left_future()
    /// } else {
    ///     ready(false).right_future()
    /// };
    ///
    /// assert_eq!(true, block_on(future));
    /// # }
    /// ```
    fn left_future<B>(self) -> Either<Self, B>
        where B: Future<Output = Self::Output>,
              Self: Sized
    {
        Either::Left(self)
    }

    /// Wrap this future in an `Either` future, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left_future` method to write `if`
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
    ///     ready(true).left_future()
    /// } else {
    ///     ready(false).right_future()
    /// };
    ///
    /// assert_eq!(false, block_on(future));
    /// # }
    fn right_future<A>(self) -> Either<A, Self>
        where A: Future<Output = Self::Output>,
              Self: Sized,
    {
        Either::Right(self)
    }*/

    /// Convert this future into a single element stream.
    ///
    /// The returned stream contains single success if this future resolves to
    /// success or single error if this future resolves into error.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::ready(17);
    /// let stream = future.into_stream();
    /// let collected: Vec<_> = block_on(stream.collect());
    /// assert_eq!(collected, vec![17]);
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
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let nested_future = future::ready(future::ready(1));
    /// let future = nested_future.flatten();
    /// assert_eq!(block_on(future), 1);
    /// # }
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Output: Future,
        Self: Sized
    {
        let f = flatten::new(self);
        assert_future::<<<Self as Future>::Output as Future>::Output, _>(f)
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
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::stream;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let stream_items = vec![17, 18, 19];
    /// let future_of_a_stream = future::ready(stream::iter(stream_items));
    ///
    /// let stream = future_of_a_stream.flatten_stream();
    /// let list: Vec<_> = block_on(stream.collect());
    /// assert_eq!(list, vec![17, 18, 19]);
    /// # }
    /// ```
    fn flatten_stream(self) -> FlattenStream<Self>
        where Self::Output: Stream,
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
        assert_future::<Self::Output, _>(f)
    }

    /// Do something with the output of a future before passing it on.
    ///
    /// When using futures, you'll often chain several of them together.  While
    /// working on such code, you might want to check out what's happening at
    /// various parts in the pipeline, without consuming the intermediate
    /// value. To do that, insert a call to `inspect`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::ready(1);
    /// let new_future = future.inspect(|&x| println!("about to resolve: {}", x));
    /// assert_eq!(block_on(new_future), 1);
    /// # }
    /// ```
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
        where F: FnOnce(&Self::Output) -> (),
              Self: Sized,
    {
        assert_future::<Self::Output, _>(inspect::new(self, f))
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
    /// use futures::prelude::*;
    /// use futures::future::{self, ReadyFuture};
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let mut future = future::ready(2);
    /// assert!(block_on(future.catch_unwind()).is_ok());
    ///
    /// let mut future = future::lazy(|_| -> ReadyFuture<i32> {
    ///     panic!();
    ///     future::ready(2)
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
    /// The returned `Shared` future resolves with `Arc<Self::Output>`,
    /// which implements `Deref` to allow shared access to the underlying
    /// result. Ownership of the underlying value cannot currently be reclaimed.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::ready(6);
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    ///
    /// assert_eq!(6, *block_on(shared1));
    /// assert_eq!(6, *block_on(shared2));
    /// # }
    /// ```
    ///
    /// ```
    /// # extern crate futures;
    /// use std::thread;
    ///
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::block_on;
    ///
    /// # fn main() {
    /// let future = future::ready(6);
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    /// let join_handle = thread::spawn(move || {
    ///     assert_eq!(6, *block_on(shared2));
    /// });
    /// assert_eq!(6, *block_on(shared1));
    /// join_handle.join().unwrap();
    /// # }
    /// ```
    #[cfg(feature = "std")]
    fn shared(self) -> Shared<Self>
        where Self: Sized
    {
        shared::new(self)
    }

    /// Assigns the provided `Executor` to be used when spawning tasks
    /// from within the future.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::future;
    /// use futures::executor::{block_on, spawn_with_handle, ThreadPool};
    ///
    /// let pool = ThreadPool::new().expect("unable to create threadpool");
    /// let future = spawn_with_handle(future::ready(3)).with_executor(pool);
    /// let output = block_on(block_on(future));
    /// assert_eq!(output, 3);
    /// ```
    #[cfg(feature = "std")]
    fn with_executor<E>(self, executor: E) -> WithExecutor<Self, E>
        where Self: Sized,
              E: ::futures_core::task::Executor
    {
        with_executor::new(self, executor)
    }
}

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
fn assert_future<A, F>(t: F) -> F
    where F: Future<Output=A>,
{
    t
}
