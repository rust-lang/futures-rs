//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};

// Primitive futures
mod empty;
pub use self::empty::{empty, Empty};

mod lazy;
pub use self::lazy::{lazy, Lazy};

mod maybe_done;
pub use self::maybe_done::{maybe_done, MaybeDone};

mod option;
pub use self::option::{OptionFuture};

mod poll_fn;
pub use self::poll_fn::{poll_fn, PollFn};

mod ready;
pub use self::ready::{ready, ok, err, Ready};

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

mod unit_error;
pub use self::unit_error::UnitError;

// Implementation details
mod chain;
pub(crate) use self::chain::Chain;

#[cfg(feature = "std")]
mod abortable;
#[cfg(feature = "std")]
pub use self::abortable::{abortable, Abortable, AbortHandle, AbortRegistration, Aborted};

#[cfg(feature = "std")]
mod catch_unwind;
#[cfg(feature = "std")]
pub use self::catch_unwind::CatchUnwind;

#[cfg(feature = "std")]
mod remote_handle;
#[cfg(feature = "std")]
pub use self::remote_handle::{Remote, RemoteHandle};

// ToDo
// #[cfg(feature = "std")]
// mod join_all;
// #[cfg(feature = "std")]
// pub use self::join_all::{join_all, JoinAll};

// #[cfg(feature = "std")]
// mod select_all;
// #[cfg(feature = "std")]
// pub use self::select_all::{SelectAll, SelectAllNext, select_all};

// #[cfg(feature = "std")]
// mod select_ok;
// #[cfg(feature = "std")]
// pub use self::select_ok::{SelectOk, select_ok};

#[cfg(feature = "std")]
mod shared;
#[cfg(feature = "std")]
pub use self::shared::Shared;

impl<T: ?Sized> FutureExt for T where T: Future {}

/// An extension trait for `Future`s that provides a variety of convenient
/// adapters.
pub trait FutureExt: Future {
    /// Map this future's output to a different type, returning a new future of
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
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let future = future::ready(1);
    /// let new_future = future.map(|x| x + 3);
    /// assert_eq!(await!(new_future), 4);
    /// # });
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
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let future_of_1 = future::ready(1);
    /// let future_of_4 = future_of_1.then(|x| future::ready(x + 3));
    /// assert_eq!(await!(future_of_4), 4);
    /// # });
    /// ```
    fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F>
        where F: FnOnce(Self::Output) -> Fut,
              Fut: Future,
              Self: Sized,
    {
        assert_future::<Fut::Output, _>(Then::new(self, f))
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
    /// }
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
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let a = future::ready(1);
    /// let b = future::ready(2);
    /// let pair = a.join(b);
    ///
    /// assert_eq!(await!(pair), (1, 2));
    /// # });
    /// ```
    fn join<Fut2>(self, other: Fut2) -> Join<Self, Fut2>
    where
        Fut2: Future,
        Self: Sized,
    {
        let f = Join::new(self, other);
        assert_future::<(Self::Output, Fut2::Output), _>(f)
    }

    /// Same as `join`, but with more futures.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let a = future::ready(1);
    /// let b = future::ready(2);
    /// let c = future::ready(3);
    /// let tuple = a.join3(b, c);
    ///
    /// assert_eq!(await!(tuple), (1, 2, 3));
    /// # });
    /// ```
    fn join3<Fut2, Fut3>(
        self,
        future2: Fut2,
        future3: Fut3,
    ) -> Join3<Self, Fut2, Fut3>
    where
        Fut2: Future,
        Fut3: Future,
        Self: Sized,
    {
        Join3::new(self, future2, future3)
    }

    /// Same as `join`, but with more futures.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let a = future::ready(1);
    /// let b = future::ready(2);
    /// let c = future::ready(3);
    /// let d = future::ready(4);
    /// let tuple = a.join4(b, c, d);
    ///
    /// assert_eq!(await!(tuple), (1, 2, 3, 4));
    /// # });
    /// ```
    fn join4<Fut2, Fut3, Fut4>(
        self,
        future2: Fut2,
        future3: Fut3,
        future4: Fut4,
    ) -> Join4<Self, Fut2, Fut3, Fut4>
    where
        Fut2: Future,
        Fut3: Future,
        Fut3: Future,
        Fut4: Future,
        Self: Sized,
    {
        Join4::new(self, future2, future3, future4)
    }

    /// Same as `join`, but with more futures.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let a = future::ready(1);
    /// let b = future::ready(2);
    /// let c = future::ready(3);
    /// let d = future::ready(4);
    /// let e = future::ready(5);
    /// let tuple = a.join5(b, c, d, e);
    ///
    /// assert_eq!(await!(tuple), (1, 2, 3, 4, 5));
    /// # });
    /// ```
    fn join5<Fut2, Fut3, Fut4, Fut5>(
        self,
        future2: Fut2,
        future3: Fut3,
        future4: Fut4,
        future5: Fut5,
    ) -> Join5<Self, Fut2, Fut3, Fut4, Fut5>
    where
        Fut2: Future,
        Fut3: Future,
        Fut3: Future,
        Fut4: Future,
        Fut5: Future,
        Self: Sized,
    {
        Join5::new(self, future2, future3, future4, future5)
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
    /// use futures::executor::block_on;
    ///
    /// let x = 6;
    /// let future = if x < 10 {
    ///     ready(true).left_future()
    /// } else {
    ///     ready(false).right_future()
    /// };
    ///
    /// assert_eq!(true, block_on(future));
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
    /// use futures::executor::block_on;
    ///
    /// let x = 6;
    /// let future = if x < 10 {
    ///     ready(true).left_future()
    /// } else {
    ///     ready(false).right_future()
    /// };
    ///
    /// assert_eq!(false, block_on(future));
    /// ```
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
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    /// use futures::stream::StreamExt;
    ///
    /// let future = future::ready(17);
    /// let stream = future.into_stream();
    /// let collected: Vec<_> = await!(stream.collect());
    /// assert_eq!(collected, vec![17]);
    /// # });
    /// ```
    fn into_stream(self) -> IntoStream<Self>
        where Self: Sized
    {
        IntoStream::new(self)
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
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let nested_future = future::ready(future::ready(1));
    /// let future = nested_future.flatten();
    /// assert_eq!(await!(future), 1);
    /// # });
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Output: Future,
        Self: Sized
    {
        let f = Flatten::new(self);
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
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream_items = vec![17, 18, 19];
    /// let future_of_a_stream = future::ready(stream::iter(stream_items));
    ///
    /// let stream = future_of_a_stream.flatten_stream();
    /// let list: Vec<_> = await!(stream.collect());
    /// assert_eq!(list, vec![17, 18, 19]);
    /// # });
    /// ```
    fn flatten_stream(self) -> FlattenStream<Self>
        where Self::Output: Stream,
              Self: Sized
    {
        FlattenStream::new(self)
    }

    /// Fuse a future such that `poll` will never again be called once it has
    /// completed. This method can be used to turn any `Future` into a
    /// `FusedFuture`.
    ///
    /// Normally, once a future has returned `Poll::Ready` from `poll`,
    /// any further calls could exhibit bad behavior such as blocking
    /// forever, panicking, never returning, etc. If it is known that `poll`
    /// may be called too often then this method can be used to ensure that it
    /// has defined semantics.
    ///
    /// If a `fuse`d future is `poll`ed after having returned `Poll::Ready`
    /// previously, it will return `Poll::Pending`, from `poll` again (and will
    /// continue to do so for all future calls to `poll`).
    ///
    /// This combinator will drop the underlying future as soon as it has been
    /// completed to ensure resources are reclaimed as soon as possible.
    fn fuse(self) -> Fuse<Self>
        where Self: Sized
    {
        let f = Fuse::new(self);
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
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let future = future::ready(1);
    /// let new_future = future.inspect(|&x| println!("about to resolve: {}", x));
    /// assert_eq!(await!(new_future), 1);
    /// # });
    /// ```
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
        where F: FnOnce(&Self::Output) -> (),
              Self: Sized,
    {
        assert_future::<Self::Output, _>(Inspect::new(self, f))
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
    // TODO: minimize and open rust-lang/rust ticket, currently errors:
    //       'assertion failed: !value.has_escaping_regions()'
    /// ```ignore
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt, Ready};
    ///
    /// let mut future = future::ready(2);
    /// assert!(await!(future.catch_unwind()).is_ok());
    ///
    /// let mut future = future::lazy(|_| -> Ready<i32> {
    ///     unimplemented!()
    /// });
    /// assert!(await!(future.catch_unwind()).is_err());
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
        where Self: Sized + ::std::panic::UnwindSafe
    {
        CatchUnwind::new(self)
    }

    /// Create a cloneable handle to this future where all handles will resolve
    /// to the same result.
    ///
    /// The `shared` combinator method provides a method to convert any future
    /// into a cloneable future. It enables a future to be polled by multiple
    /// threads.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt};
    ///
    /// let future = future::ready(6);
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    ///
    /// assert_eq!(6, await!(shared1));
    /// assert_eq!(6, await!(shared2));
    /// # });
    /// ```
    ///
    /// ```
    /// // Note, unlike most examples this is written in the context of a
    /// // synchronous function to better illustrate the cross-thread aspect of
    /// // the `shared` combinator.
    ///
    /// use futures::future::{self, FutureExt};
    /// use futures::executor::block_on;
    /// use std::thread;
    ///
    /// let future = future::ready(6);
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    /// let join_handle = thread::spawn(move || {
    ///     assert_eq!(6, block_on(shared2));
    /// });
    /// assert_eq!(6, block_on(shared1));
    /// join_handle.join().unwrap();
    /// ```
    #[cfg(feature = "std")]
    fn shared(self) -> Shared<Self>
    where
        Self: Sized,
        Self::Output: Clone,
    {
        Shared::new(self)
    }

    /// Turn this future into a future that yields `()` on completion and sends
    /// its output to another future on a separate task.
    ///
    /// This can be used with spawning executors to easily retrieve the result
    /// of a future executing on a separate task or thread.
    #[cfg(feature = "std")]
    fn remote_handle(self) -> (Remote<Self>, RemoteHandle<Self::Output>)
    where
        Self: Sized,
    {
        remote_handle::remote_handle(self)
    }

    /// Wrap the future in a Box, pinning it.
    #[cfg(feature = "std")]
    fn boxed(self) -> Pin<Box<Self>>
        where Self: Sized
    {
        Box::pinned(self)
    }

    /// Turns a `Future` into a `TryFuture` with `Error = ()`.
    fn unit_error(self) -> UnitError<Self>
        where Self: Sized
    {
        UnitError::new(self)
    }

    /// A convenience for calling `Future::poll` on `Unpin` future types.
    fn poll_unpin(&mut self, lw: &LocalWaker) -> Poll<Self::Output>
        where Self: Unpin + Sized
    {
        Pin::new(self).poll(lw)
    }
}

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
fn assert_future<T, F>(future: F) -> F
    where F: Future<Output=T>,
{
    future
}
