//! Additional combinators for testing futures.

mod assert_unmoved;
pub use self::assert_unmoved::AssertUnmoved;

mod pending_once;
pub use self::pending_once::PendingOnce;

use futures_core::future::Future;
use futures_executor;
use std::thread;

/// Additional combinators for testing futures.
pub trait FutureTestExt: Future {
    /// Asserts that the given is not moved after being polled.
    ///
    /// A check for movement is performed each time the future is polled
    /// and when `Drop` is called.
    ///
    /// Aside from keeping track of the location at which the future was first
    /// polled and providing assertions, this future adds no runtime behavior
    /// and simply delegates to the child future.
    fn assert_unmoved(self) -> AssertUnmoved<Self>
    where
        Self: Sized,
    {
        AssertUnmoved::new(self)
    }

    /// Introduces one [`Poll::Pending`](futures_core::task::Poll::Pending)
    /// before polling the given future.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await)]
    /// use futures::task::Poll;
    /// use futures::future::FutureExt;
    /// use futures_test::task::noop_context;
    /// use futures_test::future::FutureTestExt;
    /// use pin_utils::pin_mut;
    ///
    /// let future = (async { 5 }).pending_once();
    /// pin_mut!(future);
    ///
    /// let mut cx = noop_context();
    ///
    /// assert_eq!(future.poll_unpin(&mut cx), Poll::Pending);
    /// assert_eq!(future.poll_unpin(&mut cx), Poll::Ready(5));
    /// ```
    fn pending_once(self) -> PendingOnce<Self>
    where
        Self: Sized,
    {
        pending_once::PendingOnce::new(self)
    }

    /// Runs this future on a dedicated executor running in a background thread.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await)]
    /// use futures::channel::oneshot;
    /// use futures::executor::block_on;
    /// use futures_test::future::FutureTestExt;
    ///
    /// let (tx, rx) = oneshot::channel::<i32>();
    ///
    /// (async { tx.send(5).unwrap() }).run_in_background();
    ///
    /// assert_eq!(block_on(rx), Ok(5));
    /// ```
    fn run_in_background(self)
    where
        Self: Sized + Send + 'static,
        Self::Output: Send,
    {
        thread::spawn(|| futures_executor::block_on(self));
    }
}

impl<Fut> FutureTestExt for Fut where Fut: Future {}
