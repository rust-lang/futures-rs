//! Additional combinators for testing futures.

mod delay;

use self::delay::Delayed;
use futures_core::future::Future;
use futures_executor;
use std::thread;

/// Additional combinators for testing futures.
pub trait FutureTestExt: Future {
    /// Introduces one [`Poll::Pending`](futures_core::task::Poll::Pending)
    /// before polling the given future
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, futures_api, pin)]
    /// use futures::task::Poll;
    /// use futures::future::FutureExt;
    /// use futures_test::task;
    /// use futures_test::future::FutureTestExt;
    /// use pin_utils::pin_mut;
    ///
    /// let future = (async { 5 }).delay();
    /// pin_mut!(future);
    ///
    /// let cx = &mut task::no_spawn_context();
    ///
    /// assert_eq!(future.poll_unpin(cx), Poll::Pending);
    /// assert_eq!(future.poll_unpin(cx), Poll::Ready(5));
    /// ```
    fn delay(self) -> Delayed<Self>
    where
        Self: Sized,
    {
        delay::Delayed::new(self)
    }

    /// Runs this future on a dedicated executor running in a background thread.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, futures_api, pin)]
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
