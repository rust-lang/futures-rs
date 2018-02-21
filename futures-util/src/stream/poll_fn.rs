//! Definition of the `PollFn` combinator

use futures_core::{Stream, Poll};
use futures_core::task;

/// A stream which adapts a function returning `Poll`.
///
/// Created by the `poll_fn` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct PollFn<F> {
    inner: F,
}

/// Creates a new stream wrapping around a function returning `Poll`.
///
/// Polling the returned stream delegates to the wrapped function.
///
/// # Examples
///
/// ```
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::stream::poll_fn;
///
/// # fn main() {
/// let mut counter = 1usize;
///
/// let read_stream = poll_fn(move |_| -> Poll<Option<String>, std::io::Error> {
///     if counter == 0 { return Ok(Async::Ready(None)); }
///     counter -= 1;
///     Ok(Async::Ready(Some("Hello, World!".to_owned())))
/// });
/// # }
/// ```
pub fn poll_fn<T, E, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut task::Context) -> Poll<Option<T>, E>,
{
    PollFn { inner: f }
}

impl<T, E, F> Stream for PollFn<F>
where
    F: FnMut(&mut task::Context) -> Poll<Option<T>, E>,
{
    type Item = T;
    type Error = E;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<T>, E> {
        (self.inner)(cx)
    }
}
