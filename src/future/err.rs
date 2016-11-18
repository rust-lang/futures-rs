//! Definition of the Err combinator, an error value that's immediately
//! ready.

use core::marker;

use {Future, Poll};

/// A future representing a finished but erroneous computation.
///
/// Created by the `err` function.
#[must_use = "futures do nothing unless polled"]
pub struct Err<T, E> {
    _t: marker::PhantomData<T>,
    e: Option<E>,
}

/// Creates a "leaf future" from an immediate value of a failed computation.
///
/// The returned future is similar to `done` where it will immediately run a
/// scheduled callback with the provided value.
///
/// # Examples
///
/// ```
/// use futures::future::*;
///
/// let future_of_err_1 = err::<u32, u32>(1);
/// ```
pub fn err<T, E>(e: E) -> Err<T, E> {
    Err { _t: marker::PhantomData, e: Some(e) }
}

impl<T, E> Future for Err<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        Err(self.e.take().expect("cannot poll Err twice"))
    }
}
