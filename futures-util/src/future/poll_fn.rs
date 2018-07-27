//! Definition of the `PollFn` adapter combinator

use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// A future which wraps a function returning [`Poll`].
///
/// Created by the [`poll_fn()`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct PollFn<F> {
    f: F,
}

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
/// use futures::task::{self, Poll};
///
/// fn read_line(cx: &mut task::Context) -> Poll<String> {
///     Poll::Ready("Hello, World!".into())
/// }
///
/// let read_future = poll_fn(read_line);
/// assert_eq!(await!(read_future), "Hello, World!".to_owned());
/// # });
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
    where F: Unpin + FnMut(&mut task::Context) -> Poll<T>
{
    PollFn { f }
}

impl<T, F> Future for PollFn<F>
    where F: FnMut(&mut task::Context) -> Poll<T> + Unpin
{
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<T> {
        (&mut self.f)(cx)
    }
}
