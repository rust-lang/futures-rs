//! Definition of the `PollFn` adapter combinator

use anchor_experiment::MovePinned;
use futures_core::{Future, FutureMove, Poll};

/// A future which adapts a function returning `Poll`.
///
/// Created by the `poll_fn` function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct PollFn<F> {
    inner: F,
}

/// Creates a new future wrapping around a function returning `Poll`.
///
/// Polling the returned future delegates to the wrapped function.
///
/// # Examples
///
/// ```
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::future::poll_fn;
///
/// # fn main() {
/// fn read_line() -> Poll<String, std::io::Error> {
///     Ok(Async::Ready("Hello, World!".into()))
/// }
///
/// let read_future = poll_fn(read_line);
/// # }
/// ```
pub fn poll_fn<T, E, F>(f: F) -> PollFn<F>
    where F: FnMut() -> Poll<T, E>
{
    PollFn { inner: f }
}

impl<T, E, F> Future for PollFn<F>
    where F: FnMut() -> Poll<T, E>
{
    type Item = T;
    type Error = E;

    unsafe fn poll_unsafe(&mut self) -> Poll<T, E> {
        (self.inner)()
    }
}

impl<T, E, F> FutureMove for PollFn<F>
    where F: FnMut() -> Poll<T, E> + MovePinned
{
    fn poll_move(&mut self) -> Poll<T, E> {
        (self.inner)()
    }
}
