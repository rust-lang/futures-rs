//! Definition of the `PollFn` adapter combinator

use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

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
/// # #![feature(futures_api)]
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::future::poll_fn;
///
/// fn read_line(cx: &mut task::Context) -> Poll<String> {
///     Poll::Ready("Hello, World!".into())
/// }
///
/// let read_future = poll_fn(read_line);
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
    where F: Unpin + FnMut(&mut Context) -> Poll<T>
{
    PollFn { inner: f }
}

impl<T, F> Future for PollFn<F>
    where F: FnMut(&mut Context) -> Poll<T> + Unpin
{
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<T> {
        (&mut self.inner)(cx)
    }
}
