//! Asynchronous values.

use core::mem::Pin;
use core::marker::Unpin;

use {Poll, PollResult, task};

#[cfg(feature = "either")]
mod either;

/// An `Async` value represents an asychronous computation.
///
/// An `Asyn` is a value that may not have finished computing yet. This kind of
/// "asynchronous value" makes it possible for a thread to continue doing useful
/// work while it waits for the value to become available.
///
/// The ergonomics and implementation of the `Async` trait are very similar to
/// the `Iterator` trait in that there is just one method you need to
/// implement, but you get a whole lot of others for free as a result. These
/// other methods allow you to chain together large computations based on
/// async values, which will automatically handle asynchrony for you.
///
/// # The `poll` method
///
/// The core method of `Async`, `poll`, *attempts* to resolve the computation into a
/// final value. This method does not block if the value is not ready. Instead,
/// the current task is scheduled to be woken up when it's possible to make
/// further progress by `poll`ing again. The wake up is performed using
/// `cx.waker()`, a handle for waking up the current task.
///
/// When using an async value, you generally won't call `poll` directly, but instead
/// use combinators to build up asynchronous computations. A complete
/// computation can then be spawned onto an
/// [executor](../futures_core/executor/trait.Executor.html) as a new, independent
/// task that will automatically be `poll`ed to completion.
///
/// # Combinators
///
/// Like iterators, async values provide a large number of combinators to work with
/// futures to express computations in a much more natural method than
/// scheduling a number of callbacks. As with iterators, the combinators are
/// zero-cost: they compile away. You can find the combinators in the
/// [future-util](https://docs.rs/futures-util) crate.
pub trait Async {
    /// The result of the async computation
    type Output;

    /// Attempt to resolve the computation to a final value, registering
    /// the current task for wakeup if the value is not yet available.
    ///
    /// # Return value
    ///
    /// This function returns:
    ///
    /// - `Poll::Pending` if the value is not ready yet
    /// - `Poll::Ready(val)` with the result `val` on completion
    ///
    /// Once the computation has completed, clients should not `poll` it again.
    ///
    /// When an async value is not ready yet, `poll` returns
    /// [`Poll::Pending`](::Poll). Polling will will *also* register the
    /// interest of the current task in the value being produced. For example,
    /// if the computation represents the availability of data on a socket, then the
    /// task is recorded so that when data arrives, it is woken up (via
    /// [`cx.waker()`](::task::Context::waker). Once a task has been woken up,
    /// it should attempt to `poll` the async value again, which may or may not
    /// produce a final value.
    ///
    /// Note that if `Pending` is returned it only means that the *current* task
    /// (represented by the argument `cx`) will receive a notification. Tasks
    /// from previous calls to `poll` will *not* receive notifications.
    ///
    /// # Runtime characteristics
    ///
    /// Async values are *inert*; they must be *actively* `poll`ed to make
    /// progress, meaning that each time the current task is woken up, it should
    /// actively re-`poll` pending computations that it still has an interest in.
    /// Usually this is done by building up a large computation as a single
    /// async value (using combinators), then spawning that computation as a
    /// *task* onto an
    /// [executor](../futures_core/executor/trait.Executor.html). Executors
    /// ensure that each task is `poll`ed every time a computation internal to
    /// that task is ready to make progress.
    ///
    /// The `poll` function is not called repeatedly in a tight loop, but only
    /// whenever the async computation is ready to make progress, as signaled
    /// via [`cx.waker()`](::task::Context::waker). If you're familiar with the
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
    /// Once a computation has completed (returned `Ready` from `poll`),
    /// then any future calls to `poll` may panic, block forever, or otherwise
    /// cause bad behavior. The `Async` trait itself provides no guarantees
    /// about the behavior of `poll` after an `Async` has completed.
    ///
    /// Callers who may call `poll` too many times may want to consider using
    /// the `fuse` adaptor which defines the behavior of `poll`, but comes with
    /// a little bit of extra cost.
    fn poll(self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output>;
}

impl<'a, F: ?Sized + Async> Async for &'a mut F {
    type Output = F::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe { pinned_deref!(self).poll(cx) }
    }
}

if_std! {
    use std::boxed::{Box, PinBox};

    impl<'a, F: ?Sized + Async> Async for Box<F> {
        type Output = F::Output;

        fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
            unsafe { pinned_deref!(self).poll(cx) }
        }
    }

    impl<'a, F: ?Sized + Async> Async for PinBox<F> {
        type Output = F::Output;

        fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
            unsafe {
                let this = PinBox::get_mut(Pin::get_mut(&mut self));
                let re_pinned = Pin::new_unchecked(this);
                re_pinned.poll(cx)
            }
        }
    }

    impl<'a, F: Async> Async for ::std::panic::AssertUnwindSafe<F> {
        type Output = F::Output;

        fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
            unsafe { pinned_field!(self, 0).poll(cx) }
        }
    }
}

/// An immediately-ready async value
#[derive(Debug, Clone)]
#[must_use = "async values do nothing unless polled"]
pub struct Ready<T>(Option<T>);

unsafe impl<T> Unpin for Ready<T> {}

impl<T> Async for Ready<T> {
    type Output = T;

    fn poll(mut self: Pin<Self>, _cx: &mut task::Context) -> Poll<T> {
        Poll::Ready(self.0.take().unwrap())
    }
}

/// Create an immediately-ready async value.
pub fn ready<T>(t: T) -> Ready<T> {
    Ready(Some(t))
}

impl<F: Async> Async for Option<F> {
    type Output = Option<F::Output>;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe {
            match Pin::get_mut(&mut self) {
                None => Poll::Ready(None),
                Some(ref mut x) => Pin::new_unchecked(x).poll(cx).map(Some),
            }
        }
    }
}

/// A convenience for async `Result`s.
pub trait AsyncResult {
    /// The type of successful values
    type Item;

    /// The type of failures
    type Error;

    /// Poll this `AsyncResult` as if it were an `Async`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Async` trait; in the future it won't be
    /// needed.
    fn poll(self: Pin<Self>, cx: &mut task::Context) -> PollResult<Self::Item, Self::Error>;
}

impl<F, T, E> AsyncResult for F
    where F: Async<Output = Result<T, E>>
{
    type Item = T;
    type Error = E;

    fn poll(self: Pin<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        self.poll(cx)
    }
}
