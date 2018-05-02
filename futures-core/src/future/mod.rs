//! Futures.

#[cfg(feature = "nightly")]
use core::mem::Pin;

use {Unpin, Poll, task};

/// A future represents an asychronous computation.
///
/// A future is a value that may not have finished computing yet. This kind of
/// "asynchronous value" makes it possible for a thread to continue doing useful
/// work while it waits for the value to become available.
///
/// The ergonomics and implementation of the `Future` trait are very similar to
/// the `Iterator` trait in that there is just one method you need to
/// implement, but you get a whole lot of others for free as a result. These
/// other methods allow you to chain together large computations based on
/// futures, which will automatically handle asynchrony for you.
///
/// # The `poll` method
///
/// The core method of future, `poll`, *attempts* to resolve the future into a
/// final value. This method does not block if the value is not ready. Instead,
/// the current task is scheduled to be woken up when it's possible to make
/// further progress by `poll`ing again. The wake up is performed using
/// `cx.waker()`, a handle for waking up the current task.
///
/// When using a future, you generally won't call `poll` directly, but instead
/// use combinators to build up asynchronous computations. A complete
/// computation can then be spawned onto an
/// [executor](../futures_core/executor/trait.Executor.html) as a new, independent
/// task that will automatically be `poll`ed to completion.
///
/// # Combinators
///
/// Like iterators, futures provide a large number of combinators to work with
/// futures to express computations in a much more natural method than
/// scheduling a number of callbacks. As with iterators, the combinators are
/// zero-cost: they compile away. You can find the combinators in the
/// [future-util](https://docs.rs/futures-util) crate.
pub trait Future {
    /// The result of the future
    type Output;

    /// Attempt to resolve the future to a final value, registering
    /// the current task for wakeup if the value is not yet available.
    ///
    /// # Return value
    ///
    /// This function returns:
    ///
    /// - `Poll::Pending` if the future is not ready yet
    /// - `Poll::Ready(val)` with the result `val` of this future if it finished
    /// successfully.
    ///
    /// Once a future has finished, clients should not `poll` it again.
    ///
    /// When a future is not ready yet, `poll` returns
    /// [`Poll::Pending`](::Poll). The future will *also* register the
    /// interest of the current task in the value being produced. For example,
    /// if the future represents the availability of data on a socket, then the
    /// task is recorded so that when data arrives, it is woken up (via
    /// [`cx.waker()`](::task::Context::waker). Once a task has been woken up,
    /// it should attempt to `poll` the future again, which may or may not
    /// produce a final value.
    ///
    /// Note that if `Pending` is returned it only means that the *current* task
    /// (represented by the argument `cx`) will receive a notification. Tasks
    /// from previous calls to `poll` will *not* receive notifications.
    ///
    /// # Runtime characteristics
    ///
    /// Futures alone are *inert*; they must be *actively* `poll`ed to make
    /// progress, meaning that each time the current task is woken up, it should
    /// actively re-`poll` pending futures that it still has an interest in.
    /// Usually this is done by building up a large computation as a single
    /// future (using combinators), then spawning that future as a *task* onto
    /// an [executor](../futures_core/executor/trait.Executor.html). Executors
    /// ensure that each task is `poll`ed every time a future internal to that
    /// task is ready to make progress.
    ///
    /// The `poll` function is not called repeatedly in a tight loop for
    /// futures, but only whenever the future itself is ready, as signaled via
    /// [`cx.waker()`](::task::Context::waker). If you're familiar with the
    /// `poll(2)` or `select(2)` syscalls on Unix it's worth noting that futures
    /// typically do *not* suffer the same problems of "all wakeups must poll
    /// all events"; they are more like `epoll(4)`.
    ///
    /// An implementation of `poll` should strive to return quickly, and must
    /// *never* block. Returning quickly prevents unnecessarily clogging up
    /// threads or event loops. If it is known ahead of time that a call to
    /// `poll` may end up taking awhile, the work should be offloaded to a
    /// thread pool (or something similar) to ensure that `poll` can return
    /// quickly.
    ///
    /// # Panics
    ///
    /// Once a future has completed (returned `Ready` from `poll`),
    /// then any future calls to `poll` may panic, block forever, or otherwise
    /// cause bad behavior. The `Future` trait itself provides no guarantees
    /// about the behavior of `poll` after a future has completed.
    ///
    /// Callers who may call `poll` too many times may want to consider using
    /// the `fuse` adaptor which defines the behavior of `poll`, but comes with
    /// a little bit of extra cost.
    #[cfg(feature = "nightly")]
    fn poll(self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output>;

    /// A convenience for calling `poll` when a future implements `Unpin`.
    #[cfg(feature = "nightly")]
    fn poll_mut(&mut self, cx: &mut task::Context) -> Poll<Self::Output> where Self: Unpin {
        Pin::new(self).poll(cx)
    }


    /// Attempt to resolve the future to a final value, registering
    /// the current task for wakeup if the value is not yet available.
    ///
    /// # Return value
    ///
    /// This function returns:
    ///
    /// - `Poll::Pending` if the future is not ready yet
    /// - `Poll::Ready(val)` with the result `val` of this future if it finished
    /// successfully.
    ///
    /// Once a future has finished, clients should not `poll` it again.
    ///
    /// When a future is not ready yet, `poll` returns
    /// [`Poll::Pending`](::Poll). The future will *also* register the
    /// interest of the current task in the value being produced. For example,
    /// if the future represents the availability of data on a socket, then the
    /// task is recorded so that when data arrives, it is woken up (via
    /// [`cx.waker()`](::task::Context::waker). Once a task has been woken up,
    /// it should attempt to `poll` the future again, which may or may not
    /// produce a final value.
    ///
    /// Note that if `Pending` is returned it only means that the *current* task
    /// (represented by the argument `cx`) will receive a notification. Tasks
    /// from previous calls to `poll` will *not* receive notifications.
    ///
    /// # Runtime characteristics
    ///
    /// Futures alone are *inert*; they must be *actively* `poll`ed to make
    /// progress, meaning that each time the current task is woken up, it should
    /// actively re-`poll` pending futures that it still has an interest in.
    /// Usually this is done by building up a large computation as a single
    /// future (using combinators), then spawning that future as a *task* onto
    /// an [executor](../futures_core/executor/trait.Executor.html). Executors
    /// ensure that each task is `poll`ed every time a future internal to that
    /// task is ready to make progress.
    ///
    /// The `poll` function is not called repeatedly in a tight loop for
    /// futures, but only whenever the future itself is ready, as signaled via
    /// [`cx.waker()`](::task::Context::waker). If you're familiar with the
    /// `poll(2)` or `select(2)` syscalls on Unix it's worth noting that futures
    /// typically do *not* suffer the same problems of "all wakeups must poll
    /// all events"; they are more like `epoll(4)`.
    ///
    /// An implementation of `poll` should strive to return quickly, and must
    /// *never* block. Returning quickly prevents unnecessarily clogging up
    /// threads or event loops. If it is known ahead of time that a call to
    /// `poll` may end up taking awhile, the work should be offloaded to a
    /// thread pool (or something similar) to ensure that `poll` can return
    /// quickly.
    ///
    /// # Panics
    ///
    /// Once a future has completed (returned `Ready` from `poll`),
    /// then any future calls to `poll` may panic, block forever, or otherwise
    /// cause bad behavior. The `Future` trait itself provides no guarantees
    /// about the behavior of `poll` after a future has completed.
    ///
    /// Callers who may call `poll` too many times may want to consider using
    /// the `fuse` adaptor which defines the behavior of `poll`, but comes with
    /// a little bit of extra cost.
    #[cfg(not(feature = "nightly"))]
    fn poll_mut(&mut self, cx: &mut task::Context) -> Poll<Self::Output> where Self: Unpin;
}

/// A convenience for futures that return `Result` values.
pub trait TryFuture {
    /// The type of successful values yielded by this future
    type Item;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; eventually it won't be
    /// needed.
    #[cfg(feature = "nightly")]
    fn try_poll(self: Pin<Self>, cx: &mut task::Context) -> Poll<Result<Self::Item, Self::Error>>;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; eventually it won't be
    /// needed.
    fn try_poll_mut(&mut self, cx: &mut task::Context) -> Poll<Result<Self::Item, Self::Error>>
        where Self: Unpin;
}

impl<F, T, E> TryFuture for F
    where F: Future<Output = Result<T, E>>
{
    type Item = T;
    type Error = E;

    #[cfg(feature = "nightly")]
    fn try_poll(self: Pin<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        self.poll(cx)
    }

    fn try_poll_mut(&mut self, cx: &mut task::Context) -> Poll<F::Output>
        where Self: Unpin
    {
        self.poll_mut(cx)
    }
}


/// A future that is immediately ready with a value
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct ReadyFuture<T>(Option<T>);

unsafe impl<T> Unpin for ReadyFuture<T> {}

impl<T> Future for ReadyFuture<T> {
    type Output = T;

    #[cfg(feature = "nightly")]
    fn poll(mut self: Pin<Self>, _cx: &mut task::Context) -> Poll<T> {
        Poll::Ready(self.0.take().unwrap())
    }

    #[cfg(not(feature = "nightly"))]
    fn poll_mut(&mut self, _cx: &mut task::Context) -> Poll<T> {
        Poll::Ready(self.0.take().unwrap())
    }
}

/// Create a future that is immediately ready with a value.
pub fn ready<T>(t: T) -> ReadyFuture<T> {
    ReadyFuture(Some(t))
}
