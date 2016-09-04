//! Futures at zero cost
//!
//! This library is an implementation of futures in Rust which aims to provide
//! zero cost abstractions, `Iterator`-like ergonomics, and easy composability
//! between `Future`-related APIs.
//!
//! Futures are a concept for an object which is a proxy for another value that
//! may not be ready yet. For example issuing an HTTP request may return a
//! future for the HTTP response, as it probably hasn't arrived yet. With an
//! object representing a value that will eventually be available, futures allow
//! for powerful composition of tasks through basic combinators that can perform
//! operations like chaining computations, changing the types of futures, or
//! waiting for two futures to complete at the same time.
//!
//! ## Installation
//!
//! Currently it's recommended to use the git version of this repository as it's
//! in active development, but this will be published to crates.io in the near
//! future!
//!
//! ```toml
//! [dependencies]
//! futures = { git = "https://github.com/alexcrichton/futures-rs" }
//! ```
//!
//! ## Examples
//!
//! Let's take a look at a few examples of how futures might be used:
//!
//! ```
//! extern crate futures;
//!
//! use std::io;
//! use std::time::Duration;
//! use futures::{Future, Map};
//!
//! // A future is actually a trait implementation, so we can generically take a
//! // future of any integer and return back a future that will resolve to that
//! // value plus 10 more.
//! //
//! // Note here that like iterators, we're returning the `Map` combinator in
//! // the futures crate, not a boxed abstraction. This is a zero-cost
//! // construction of a future.
//! fn add_ten<F>(future: F) -> Map<F, fn(i32) -> i32>
//!     where F: Future<Item=i32>,
//! {
//!     fn add(a: i32) -> i32 { a + 10 }
//!     future.map(add)
//! }
//!
//! // Not only can we modify one future, but we can even compose them together!
//! // Here we have a function which takes two futures as input, and returns a
//! // future that will calculate the sum of their two values.
//! //
//! // Above we saw a direct return value of the `Map` combinator, but
//! // performance isn't always critical and sometimes it's more ergonomic to
//! // return a trait object like we do here. Note though that there's only one
//! // allocation here, not any for the intermediate futures.
//! fn add<'a, A, B>(a: A, b: B) -> Box<Future<Item=i32, Error=A::Error> + 'a>
//!     where A: Future<Item=i32> + 'a,
//!           B: Future<Item=i32, Error=A::Error> + 'a,
//! {
//!     Box::new(a.join(b).map(|(a, b)| a + b))
//! }
//!
//! // Futures also allow chaining computations together, starting another after
//! // the previous finishes. Here we wait for the first computation to finish,
//! // and then decide what to do depending on the result.
//! fn download_timeout(url: &str,
//!                     timeout_dur: Duration)
//!                     -> Box<Future<Item=Vec<u8>, Error=io::Error>> {
//!     use std::io;
//!     use std::net::{SocketAddr, TcpStream};
//!
//!     type IoFuture<T> = Box<Future<Item=T, Error=io::Error>>;
//!
//!     // First thing to do is we need to resolve our URL to an address. This
//!     // will likely perform a DNS lookup which may take some time.
//!     let addr = resolve(url);
//!
//!     // After we acquire the address, we next want to open up a TCP
//!     // connection.
//!     let tcp = addr.and_then(|addr| connect(&addr));
//!
//!     // After the TCP connection is established and ready to go, we're off to
//!     // the races!
//!     let data = tcp.and_then(|conn| download(conn));
//!
//!     // That all might take awhile, though, so let's not wait too long for it
//!     // to all come back. The `select` combinator here returns a future which
//!     // resolves to the first value that's ready plus the next future.
//!     //
//!     // Note we can also use the `then` combinator which which is similar to
//!     // `and_then` above except that it receives the result of the
//!     // computation, not just the successful value.
//!     //
//!     // Again note that all the above calls to `and_then` and the below calls
//!     // to `map` and such require no allocations. We only ever allocate once
//!     // we hit the `.boxed()` call at the end here, which means we've built
//!     // up a relatively involved computation with only one box, and even that
//!     // was optional!
//!
//!     let data = data.map(Ok);
//!     let timeout = timeout(timeout_dur).map(Err);
//!
//!     let ret = data.select(timeout).then(|result| {
//!         match result {
//!             // One future succeeded, and it was the one which was
//!             // downloading data from the connection.
//!             Ok((Ok(data), _other_future)) => Ok(data),
//!
//!             // The timeout fired, and otherwise no error was found, so
//!             // we translate this to an error.
//!             Ok((Err(_timeout), _other_future)) => {
//!                 Err(io::Error::new(io::ErrorKind::Other, "timeout"))
//!             }
//!
//!             // A normal I/O error happened, so we pass that on through.
//!             Err((e, _other_future)) => Err(e),
//!         }
//!     });
//!     return Box::new(ret);
//!
//!     fn resolve(url: &str) -> IoFuture<SocketAddr> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//!
//!     fn connect(hostname: &SocketAddr) -> IoFuture<TcpStream> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//!
//!     fn download(stream: TcpStream) -> IoFuture<Vec<u8>> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//!
//!     fn timeout(stream: Duration) -> IoFuture<()> {
//!         // ...
//! #       panic!("unimplemented");
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! Some more information can also be found in the [README] for now, but
//! otherwise feel free to jump in to the docs below!
//!
//! [README]: https://github.com/alexcrichton/futures-rs#futures-rs

#![no_std]
#![deny(missing_docs)]
#![cfg_attr(feature = "never", feature(never_type, default_type_parameter_fallback))]

#[macro_use]
#[cfg(feature = "use_std")]
extern crate std;

#[macro_use]
extern crate log;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "use_std")]
        $i
    )*)
}

#[macro_use]
mod poll;
pub use poll::{Poll, Async};

// Primitive futures
mod done;
mod empty;
mod failed;
mod finished;
mod lazy;
#[cfg(feature = "never")]
mod never;
pub use done::{done, Done};
pub use empty::{empty, Empty};
pub use failed::{failed, Failed};
pub use finished::{finished, Finished};
pub use lazy::{lazy, Lazy};
#[cfg(feature = "never")]
pub use never::{
    never, Never,
    always_failed, AlwaysFailed,
    always_finished, AlwaysFinished
};

// combinators
mod and_then;
mod flatten;
mod fuse;
mod join;
mod map;
mod map_err;
mod or_else;
mod select;
mod then;
pub use and_then::AndThen;
pub use flatten::Flatten;
pub use fuse::Fuse;
pub use join::{Join, Join3, Join4, Join5};
pub use map::Map;
pub use map_err::MapErr;
pub use or_else::OrElse;
pub use select::{Select, SelectNext};
pub use then::Then;

if_std! {
    mod lock;
    mod slot;
    pub mod task;

    mod catch_unwind;
    mod collect;
    mod oneshot;
    mod select_all;
    pub use catch_unwind::CatchUnwind;
    pub use collect::{collect, Collect};
    pub use oneshot::{oneshot, Oneshot, Complete, Canceled};
    pub use select_all::{SelectAll, SelectAllNext, select_all};

    /// A type alias for `Box<Future + Send>`
    pub type BoxFuture<T, E> = std::boxed::Box<Future<Item = T, Error = E> + Send>;

    impl<F: ?Sized + Future> Future for std::boxed::Box<F> {
        type Item = F::Item;
        type Error = F::Error;

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            (**self).poll()
        }
    }
}

// streams
pub mod stream;

// impl details
mod chain;

/// Trait for types which are a placeholder of a value that will become
/// available at possible some later point in time.
///
/// Futures are used to provide a sentinel through which a value can be
/// referenced. They crucially allow chaining and composing operations through
/// consumption which allows expressing entire trees of computation as one
/// sentinel value.
///
/// The ergonomics and implementation of the `Future` trait are very similar to
/// the `Iterator` trait in Rust which is where there is a small handful of
/// methods to implement and a load of default methods that consume a `Future`,
/// producing a new value.
///
/// # The `poll` method
///
/// The core method of future, `poll`, is not intended to be called in general.
/// The `poll` method is typically called in the context of a "task" which
/// drives a future to completion. For more information on this see the `task`
/// module.
///
/// # Combinators
///
/// Like iterators, futures provide a large number of combinators to work with
/// futures to express computations in a much more natural method than
/// scheduling a number of callbacks. For example the `map` method can change
/// a `Future<Item=T>` to a `Future<Item=U>` or an `and_then` combinator could
/// create a future after the first one is done and only be resolved when the
/// second is done.
///
/// Combinators act very similarly to the methods on the `Iterator` trait itself
/// or those on `Option` and `Result`. Like with iterators, the combinators are
/// zero-cost and don't impose any extra layers of indirection you wouldn't
/// otherwise have to write down.
// TODO: expand this
pub trait Future {

    /// The type of value that this future will resolved with if it is
    /// successful.
    type Item;

    /// The type of error that this future will resolve with if it fails in a
    /// normal fashion.
    ///
    /// Futures may also fail due to panics or cancellation, but that is
    /// expressed through the `PollError` type, not this type.
    type Error;

    /// Query this future to see if its value has become available, registering
    /// interest if it is not.
    ///
    /// This function will check the internal state of the future and assess
    /// whether the value is ready to be produced. Implementors of this function
    /// should ensure that a call to this **never blocks** as event loops may
    /// not work properly otherwise.
    ///
    /// When a future is not ready yet, the `Async::NotReady` value will be
    /// returned. In this situation the future will *also* register interest of
    /// the current task in the value being produced. That is, once the value is
    /// ready it will notify the current task that progress can be made.
    ///
    /// # Runtime characteristics
    ///
    /// This function, `poll`, is the primary method for 'making progress'
    /// within a tree of futures. For example this method will be called
    /// repeatedly as the internal state machine makes its various transitions.
    /// Additionally, this function may not necessarily have many guarantees
    /// about *where* it's run (e.g. always on an I/O thread or not). Unless it
    /// is otherwise arranged to be so, it should be ensured that
    /// **implementations of this function finish very quickly**.
    ///
    /// This prevents unnecessarily clogging up threads and/or event loops while
    /// a `poll` function call, for example, takes up compute resources to
    /// perform some expensive computation. If it is known ahead of time that a
    /// call to `poll` may end up taking awhile, the work should be offloaded to
    /// a thread pool (or something similar) to ensure that `poll` can return
    /// quickly.
    ///
    /// # Return value
    ///
    /// This function returns `Async::NotReady` if the future is not ready yet,
    /// `Err` if the future is finished but resolved to an error, or
    /// `Async::Ready` with the result of this future if it's finished
    /// successfully. Once a future has finished it is considered a contract
    /// error to continue polling it.
    ///
    /// If `NotReady` is returned, then the future will internally register
    /// interest in the value being produced for the current task. In other
    /// words, the current task will receive a notification once the value is
    /// ready to be produced or the future can make progress.
    ///
    /// # Panics
    ///
    /// Once a future has completed (returned `Ready` or `Err` from `poll`),
    /// then any future calls to `poll` may panic, block forever, or otherwise
    /// cause wrong behavior. The `Future` trait itself provides no guarantees
    /// about the behavior of `poll` after a future has completed.
    ///
    /// Callers who may call `poll` too many times may want to consider using
    /// the `fuse` adaptor which defines the behavior of `poll`, but comes with
    /// a little bit of extra cost.
    ///
    /// Additionally, calls to `poll` must always be made from within the
    /// context of a task. If a current task is not set then this method will
    /// likely panic.
    ///
    /// # Errors
    ///
    /// This future may have failed to finish the computation, in which case
    /// the `Err` variant will be returned with an appropriate payload of an
    /// error.
    fn poll(&mut self) -> Poll<Self::Item, Self::Error>;

    /// Block the current thread until this future is resolved.
    ///
    /// This method will consume ownership of this future, driving it to
    /// completion via `poll` and blocking the current thread while it's waiting
    /// for the value to become available. Once the future is resolved the
    /// result of this future is returned.
    ///
    /// > **Note:** This method is not appropriate to call on event loops or
    /// >           similar I/O situations because it will prevent the event
    /// >           loop from making progress (this blocks the thread). This
    /// >           method should only be called when it's guaranteed that the
    /// >           blocking work associated with this future will be completed
    /// >           by another thread.
    ///
    /// # Behavior
    ///
    /// This function will *pin* this future to the current thread. The future
    /// will only be polled by this thread.
    ///
    /// # Panics
    ///
    /// This function does not attempt to catch panics. If the `poll` function
    /// panics, panics will be propagated to the caller.
    #[cfg(feature = "use_std")]
    fn wait(self) -> Result<Self::Item, Self::Error>
        where Self: Sized
    {
        task::spawn(self).wait_future()
    }

    /// Convenience function for turning this future into a trait object.
    ///
    /// This simply avoids the need to write `Box::new` and can often help with
    /// type inference as well by always returning a trait object. Note that
    /// this method requires the `Send` bound and returns a `BoxFuture`, which
    /// also encodes this. If you'd like to create a `Box<Future>` without the
    /// `Send` bound, then the `Box::new` function can be used instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let a: BoxFuture<i32, i32> = done(Ok(1)).boxed();
    /// ```
    #[cfg(feature = "use_std")]
    fn boxed(self) -> BoxFuture<Self::Item, Self::Error>
        where Self: Sized + Send + 'static
    {
        ::std::boxed::Box::new(self)
    }

    /// Map this future's result to a different type, returning a new future of
    /// the resulting type.
    ///
    /// This function is similar to the `Option::map` or `Iterator::map` where
    /// it will change the type of the underlying future. This is useful to
    /// chain along a computation once a future has been resolved.
    ///
    /// The closure provided will only be called if this future is resolved
    /// successfully. If this future returns an error, panics, or is canceled,
    /// then the closure provided will never be invoked.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let future_of_1 = finished::<u32, u32>(1);
    /// let future_of_4 = future_of_1.map(|x| x + 3);
    /// ```
    fn map<F, U>(self, f: F) -> Map<Self, F>
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
    /// canceled, then the closure provided will never be invoked.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let future_of_err_1 = failed::<u32, u32>(1);
    /// let future_of_err_4 = future_of_err_1.map_err(|x| x + 3);
    /// ```
    fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
        where F: FnOnce(Self::Error) -> E,
              Self: Sized,
    {
        assert_future::<Self::Item, E, _>(map_err::new(self, f))
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
    /// If this future is canceled or panics then the closure `f` will not be
    /// run.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let future_of_1 = finished::<u32, u32>(1);
    /// let future_of_4 = future_of_1.then(|x| {
    ///     x.map(|y| y + 3)
    /// });
    ///
    /// let future_of_err_1 = failed::<u32, u32>(1);
    /// let future_of_4 = future_of_err_1.then(|x| {
    ///     match x {
    ///         Ok(_) => panic!("expected an error"),
    ///         Err(y) => finished::<u32, u32>(y + 3),
    ///     }
    /// });
    /// ```
    fn then<F, B>(self, f: F) -> Then<Self, B, F>
        where F: FnOnce(Result<Self::Item, Self::Error>) -> B,
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
    /// If this future is canceled, panics, or completes with an error then the
    /// provided closure `f` is never called.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let future_of_1 = finished::<u32, u32>(1);
    /// let future_of_4 = future_of_1.and_then(|x| {
    ///     Ok(x + 3)
    /// });
    ///
    /// let future_of_err_1 = failed::<u32, u32>(1);
    /// future_of_err_1.and_then(|_| -> Done<u32, u32> {
    ///     panic!("should not be called in case of an error");
    /// });
    /// ```
    fn and_then<F, B>(self, f: F) -> AndThen<Self, B, F>
        where F: FnOnce(Self::Item) -> B,
              B: IntoFuture<Error = Self::Error>,
              Self: Sized,
    {
        assert_future::<B::Item, Self::Error, _>(and_then::new(self, f))
    }

    /// Execute another future after this one has resolved with an error.
    ///
    /// This function can be used to chain two futures together and ensure that
    /// the final future isn't resolved until both have finished. The closure
    /// provided is yielded the error of this future and returns another value
    /// which can be converted into a future.
    ///
    /// Note that because `Result` implements the `IntoFuture` trait this method
    /// can also be useful for chaining fallible and serial computations onto
    /// the end of one future.
    ///
    /// If this future is canceled, panics, or completes successfully then the
    /// provided closure `f` is never called.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let future_of_err_1 = failed::<u32, u32>(1);
    /// let future_of_4 = future_of_err_1.or_else(|x| -> Result<u32, u32> {
    ///     Ok(x + 3)
    /// });
    ///
    /// let future_of_1 = finished::<u32, u32>(1);
    /// future_of_1.or_else(|_| -> Done<u32, u32> {
    ///     panic!("should not be called in case of success");
    /// });
    /// ```
    fn or_else<F, B>(self, f: F) -> OrElse<Self, B, F>
        where F: FnOnce(Self::Error) -> B,
              B: IntoFuture<Item = Self::Item>,
              Self: Sized,
    {
        assert_future::<Self::Item, B::Error, _>(or_else::new(self, f))
    }

    /// Waits for either one of two futures to complete.
    ///
    /// This function will return a new future which awaits for either this or
    /// the `other` future to complete. The returned future will finish with
    /// both the value resolved and a future representing the completion of the
    /// other work. Both futures must have the same item and error type.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// // A poor-man's join implemented on top of select
    ///
    /// fn join<A>(a: A, b: A) -> BoxFuture<(u32, u32), u32>
    ///     where A: Future<Item = u32, Error = u32> + Send + 'static,
    /// {
    ///     a.select(b).then(|res| {
    ///         match res {
    ///             Ok((a, b)) => b.map(move |b| (a, b)).boxed(),
    ///             Err((a, _)) => failed(a).boxed(),
    ///         }
    ///     }).boxed()
    /// }
    /// ```
    fn select<B>(self, other: B) -> Select<Self, B::Future>
        where B: IntoFuture<Item=Self::Item, Error=Self::Error>,
              Self: Sized,
    {
        let f = select::new(self, other.into_future());
        assert_future::<(Self::Item, SelectNext<Self, B::Future>),
                        (Self::Error, SelectNext<Self, B::Future>), _>(f)
    }

    /// Joins the result of two futures, waiting for them both to complete.
    ///
    /// This function will return a new future which awaits both this and the
    /// `other` future to complete. The returned future will finish with a tuple
    /// of both results.
    ///
    /// Both futures must have the same error type, and if either finishes with
    /// an error then the other will be canceled and that error will be
    /// returned.
    ///
    /// If either future is canceled or panics, the other is canceled and the
    /// original error is propagated upwards.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let a = finished::<u32, u32>(1);
    /// let b = finished::<u32, u32>(2);
    /// let pair = a.join(b);
    ///
    /// pair.map(|(a, b)| {
    ///     assert_eq!(a, 1);
    ///     assert_eq!(b, 2);
    /// });
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

    /// Flatten the execution of this future when the successful result of this
    /// future is itself another future.
    ///
    /// This can be useful when combining futures together to flatten the
    /// computation out the the final result. This method can only be called
    /// when the successful result of this future itself implements the
    /// `IntoFuture` trait and the error can be created from this future's error
    /// type.
    ///
    /// This method is equivalent to `self.then(|x| x)`.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::*;
    ///
    /// let future_of_a_future = finished::<_, u32>(finished::<u32, u32>(1));
    /// let future_of_1 = future_of_a_future.flatten();
    /// ```
    fn flatten(self) -> Flatten<Self>
        where Self::Item: IntoFuture,
              <<Self as Future>::Item as IntoFuture>::Error:
                    From<<Self as Future>::Error>,
              Self: Sized
    {
        let f = flatten::new(self);
        assert_future::<<<Self as Future>::Item as IntoFuture>::Item,
                        <<Self as Future>::Item as IntoFuture>::Error,
                        _>(f)
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
    /// then it will forever return `NotReady` from `poll` again (never
    /// resolve).  This, unlike the trait's `poll` method, is guaranteed.
    ///
    /// Additionally, once a future has completed, this `Fuse` combinator will
    /// ensure that all registered callbacks will not be registered with the
    /// underlying future.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use futures::*;
    ///
    /// let mut future = finished::<i32, u32>(2);
    /// assert_eq!(future.poll(), Ok(Async::Ready(2)));
    ///
    /// // Normally, a call such as this would panic:
    /// //future.poll();
    ///
    /// // This, however, is guaranteed to not panic
    /// let mut future = finished::<i32, u32>(2).fuse();
    /// assert_eq!(future.poll(), Ok(Async::Ready(2)));
    /// assert_eq!(future.poll(), Ok(Async::NotReady));
    /// ```
    fn fuse(self) -> Fuse<Self>
        where Self: Sized
    {
        let f = fuse::new(self);
        assert_future::<Self::Item, Self::Error, _>(f)
    }

    /// Catches unwinding panics while polling the future.
    ///
    /// In general, panics within a future can propagate all the way out to the
    /// task level. This combinator makes it possible to halt unwinding within
    /// the future itself. It's most commonly used within task executors.
    ///
    /// Note that this method requires the `UnwindSafe` bound from the standard
    /// library. This isn't always applied automatically, and the standard
    /// library provides an `AssertUnwindSafe` wrapper type to apply it
    /// after-the fact. To assist using this method, the `Future` trait is also
    /// implemented for `AssertUnwindSafe<F>` where `F` implements `Future`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use futures::*;
    ///
    /// let mut future = finished::<i32, u32>(2);
    /// assert!(future.catch_unwind().wait().is_ok());
    ///
    /// let mut future = lazy(|| {
    ///     panic!();
    ///     finished::<i32, u32>(2)
    /// });
    /// assert!(future.catch_unwind().wait().is_err());
    /// ```
    #[cfg(feature = "use_std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
        where Self: Sized + std::panic::UnwindSafe
    {
        catch_unwind::new(self)
    }
}

impl<'a, F: ?Sized + Future> Future for &'a mut F {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        (**self).poll()
    }
}

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
fn assert_future<A, B, F>(t: F) -> F
    where F: Future<Item=A, Error=B>,
{
    t
}

/// Class of types which can be converted themselves into a future.
///
/// This trait is very similar to the `IntoIterator` trait and is intended to be
/// used in a very similar fashion.
pub trait IntoFuture {
    /// The future that this type can be converted into.
    type Future: Future<Item=Self::Item, Error=Self::Error>;

    /// The item that the future may resolve with.
    type Item;
    /// The error that the future may resolve with.
    type Error;

    /// Consumes this object and produces a future.
    fn into_future(self) -> Self::Future;
}

impl<F: Future> IntoFuture for F {
    type Future = F;
    type Item = F::Item;
    type Error = F::Error;

    fn into_future(self) -> F {
        self
    }
}

impl<T, E> IntoFuture for Result<T, E> {
    type Future = Done<T, E>;
    type Item = T;
    type Error = E;

    fn into_future(self) -> Done<T, E> {
        done(self)
    }
}
