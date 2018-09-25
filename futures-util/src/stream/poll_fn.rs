//! Definition of the `PollFn` combinator

use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// A stream which adapts a function returning `Poll`.
///
/// Created by the `poll_fn` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct PollFn<F> {
    f: F,
}

impl<F> Unpin for PollFn<F> {}

/// Creates a new stream wrapping around a function returning `Poll`.
///
/// Polling the returned stream delegates to the wrapped function.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures::stream::poll_fn;
/// use futures::task::Poll;
///
/// let mut counter = 1usize;
///
/// let read_stream = poll_fn(move |_| -> Poll<Option<String>> {
///     if counter == 0 { return Poll::Ready(None); }
///     counter -= 1;
///     Poll::Ready(Some("Hello, World!".to_owned()))
/// });
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&LocalWaker) -> Poll<Option<T>>,
{
    PollFn { f }
}

impl<T, F> Stream for PollFn<F>
where
    F: FnMut(&LocalWaker) -> Poll<Option<T>>,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<T>> {
        (&mut self.f)(lw)
    }
}
