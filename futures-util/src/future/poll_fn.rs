//! Definition of the `PollFn` adapter combinator

use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};

/// Future for the [`poll_fn`] combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct PollFn<F> {
    f: F,
}

impl<F> Unpin for PollFn<F> {}

/// Creates a new future wrapping around a function returning [`Poll`].
///
/// Polling the returned future delegates to the wrapped function.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future::poll_fn;
/// use futures::task::{Waker, Poll};
///
/// fn read_line(waker: &Waker) -> Poll<String> {
///     Poll::Ready("Hello, World!".into())
/// }
///
/// let read_future = poll_fn(read_line);
/// assert_eq!(await!(read_future), "Hello, World!".to_owned());
/// # });
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&Waker) -> Poll<T>
{
    PollFn { f }
}

impl<T, F> Future for PollFn<F>
    where F: FnMut(&Waker) -> Poll<T>,
{
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<T> {
        (&mut self.f)(waker)
    }
}
