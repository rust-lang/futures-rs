//! Definition of the Lazy combinator, deferring execution of a function until
//! the future is polled.

use core::mem::PinMut;
use core::marker::Unpin;

use futures_core::{Future, Poll};
use futures_core::task;

/// A future which, when polled, invokes a closure and yields its result.
///
/// This is created by the `lazy` function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Lazy<F> {
    f: Option<F>
}

// safe because we never generate `PinMut<F>`
unsafe impl<F> Unpin for Lazy<F> {}

/// Creates a new future from a closure.
///
/// The provided closure is only run once the future is polled.
///
/// # Examples
///
/// ```
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::future;
///
/// # fn main() {
/// let a = future::lazy(|_| 1);
///
/// let b = future::lazy(|_| -> i32 {
///     panic!("oh no!")
/// });
/// drop(b); // closure is never run
/// # }
/// ```
pub fn lazy<F, R>(f: F) -> Lazy<F>
    where F: FnOnce(&mut task::Context) -> R,
{
    Lazy { f: Some(f) }
}

impl<R, F> Future for Lazy<F>
    where F: FnOnce(&mut task::Context) -> R,
{
    type Output = R;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<R> {
        Poll::Ready((self.f.take().unwrap())(cx))
    }
}
