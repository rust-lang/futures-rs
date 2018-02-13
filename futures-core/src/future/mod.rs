//! Futures
//!
//! This module contains the `Future` and `IntoFuture` traits.
//! Implementations are also provided for core types like
//! `Box`, `Option`, and `Result`.

use Poll;
use task;

mod option;
#[path = "result.rs"]
mod result_;
pub use self::result_::{result, ok, err, Result};

/// Trait for types which are a placeholder of a value that may become
/// available at some later point in time.
///
/// In addition to the documentation here you can also find more information
/// about futures [online] at [https://tokio.rs](https://tokio.rs)
///
/// [online]: https://tokio.rs/docs/getting-started/futures/
///
/// Futures are used to provide a sentinel through which a value can be
/// referenced. They crucially allow chaining and composing operations through
/// consumption which allows expressing entire trees of computation as one
/// sentinel value.
///
/// The ergonomics and implementation of the `Future` trait are very similar to
/// the `Iterator` trait in that there is just one methods you need
/// to implement, but you get a whole lot of others for free as a result.
///
/// # The `poll` method
///
/// The core method of future, `poll`, is used to attempt to generate the value
/// of a `Future`. This method *does not block* but is allowed to inform the
/// caller that the value is not ready yet. Implementations of `poll` may
/// themselves do work to generate the value, but it's guaranteed that this will
/// never block the calling thread.
///
/// A key aspect of this method is that if the value is not yet available the
/// current task is scheduled to receive a notification when it's later ready to
/// be made available. This follows what's typically known as a "readiness" or
/// "pull" model where values are pulled out of futures on demand, and
/// otherwise a task is notified when a value might be ready to get pulled out.
///
/// The `poll` method is not intended to be called in general, but rather is
/// typically called in the context of a "task" which drives a future to
/// completion. For more information on this see the `task` module.
///
/// More information about the details of `poll` and the nitty-gritty of tasks
/// can be [found online at tokio.rs][poll-dox].
///
/// [poll-dox]: https://tokio.rs/docs/going-deeper-futures/futures-model/
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
///
/// More information about combinators can be found [on tokio.rs].
///
/// [on tokio.rs]: https://tokio.rs/docs/going-deeper-futures/futures-mechanics/
pub trait Future {
    /// The type of value that this future will resolved with if it is
    /// successful.
    type Item;

    /// The type of error that this future will resolve with if it fails in a
    /// normal fashion.
    type Error;

    /// Query this future to see if its value has become available, registering
    /// interest if it is not.
    ///
    /// This function will check the internal state of the future and assess
    /// whether the value is ready to be produced. Implementers of this function
    /// should ensure that a call to this **never blocks** as event loops may
    /// not work properly otherwise.
    ///
    /// When a future is not ready yet, the `Async::Pending` value will be
    /// returned. In this situation the future will *also* register interest of
    /// the current task in the value being produced. This is done by calling
    /// `task::park` to retrieve a handle to the current `Task`. When the future
    /// is then ready to make progress (e.g. it should be `poll`ed again) the
    /// `unpark` method is called on the `Task`.
    ///
    /// More information about the details of `poll` and the nitty-gritty of
    /// tasks can be [found online at tokio.rs][poll-dox].
    ///
    /// [poll-dox]: https://tokio.rs/docs/going-deeper-futures/futures-model/
    ///
    /// # Runtime characteristics
    ///
    /// This function, `poll`, is the primary method for 'making progress'
    /// within a tree of futures. For example this method will be called
    /// repeatedly as the internal state machine makes its various transitions.
    /// Executors are responsible for ensuring that this function is called in
    /// the right location (e.g. always on an I/O thread or not). Unless it is
    /// otherwise arranged to be so, it should be ensured that **implementations
    /// of this function finish very quickly**.
    ///
    /// Returning quickly prevents unnecessarily clogging up threads and/or
    /// event loops while a `poll` function call, for example, takes up compute
    /// resources to perform some expensive computation. If it is known ahead
    /// of time that a call to `poll` may end up taking awhile, the work should
    /// be offloaded to a thread pool (or something similar) to ensure that
    /// `poll` can return quickly.
    ///
    /// Note that the `poll` function is not called repeatedly in a loop for
    /// futures typically, but only whenever the future itself is ready. If
    /// you're familiar with the `poll(2)` or `select(2)` syscalls on Unix
    /// it's worth noting that futures typically do *not* suffer the same
    /// problems of "all wakeups must poll all events". Futures have enough
    /// support for only polling futures which cause a wakeup.
    ///
    /// # Return value
    ///
    /// This function returns `Async::Pending` if the future is not ready yet,
    /// `Err` if the future is finished but resolved to an error, or
    /// `Async::Ready` with the result of this future if it's finished
    /// successfully. Once a future has finished it is considered a contract
    /// error to continue polling the future.
    ///
    /// If `Pending` is returned, then the future will internally register
    /// interest in the value being produced for the current task (through
    /// `task::park`). In other words, the current task will receive a
    /// notification (through the `unpark` method) once the value is ready to be
    /// produced or the future can make progress.
    ///
    /// Note that if `Pending` is returned it only means that *this* task will
    /// receive a notification. Historical calls to `poll` with different tasks
    /// will not receive notifications. In other words, implementers of the
    /// `Future` trait need not store a queue of tasks to notify, but only the
    /// last task that called this method. Alternatively callers of this method
    /// can only rely on the most recent task which call `poll` being notified
    /// when a future is ready.
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
    fn poll(&mut self, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error>;
}

impl<'a, F: ?Sized + Future> Future for &'a mut F {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        (**self).poll(ctx)
    }
}

if_std! {
    impl<F: ?Sized + Future> Future for ::std::boxed::Box<F> {
        type Item = F::Item;
        type Error = F::Error;

        fn poll(&mut self, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
            (**self).poll(ctx)
        }
    }


    impl<F: Future> Future for ::std::panic::AssertUnwindSafe<F> {
        type Item = F::Item;
        type Error = F::Error;

        fn poll(&mut self, ctx: &mut task::Context) -> Poll<F::Item, F::Error> {
            self.0.poll(ctx)
        }
    }
}

/// Class of types which can be converted into a future.
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

impl<F> IntoFuture for F where F: Future {
    type Future = Self;
    type Item = <Self as Future>::Item;
    type Error = <Self as Future>::Error;
    fn into_future(self) -> Self { self }
}

/// Asynchronous conversion from a type `T`.
///
/// This trait is analogous to `std::convert::From`, adapted to asynchronous
/// computation.
pub trait FutureFrom<T>: Sized {
    /// The future for the conversion.
    type Future: Future<Item=Self, Error=Self::Error>;

    /// Possible errors during conversion.
    type Error;

    /// Consume the given value, beginning the conversion.
    fn future_from(T) -> Self::Future;
}
