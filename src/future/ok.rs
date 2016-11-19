//! Definition of the Ok combinator, a successful value that's immediately
//! ready.

use core::marker;

use {Future, Poll, Async};

/// A future representing a finished successful computation.
///
/// Created by the `finished` function.
#[must_use = "futures do nothing unless polled"]
pub struct Ok<T, E> {
    t: Option<T>,
    _e: marker::PhantomData<E>,
}

/// Creates a "leaf future" from an immediate value of a finished and
/// successful computation.
///
/// The returned future is similar to `done` where it will immediately run a
/// scheduled callback with the provided value.
///
/// # Examples
///
/// ```
/// use futures::future::*;
///
/// let future_of_1 = ok::<u32, u32>(1);
/// ```
pub fn ok<T, E>(t: T) -> Ok<T, E> {
    Ok { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for Ok<T, E> {
    type Item = T;
    type Error = E;


    fn poll(&mut self) -> Poll<T, E> {
        Ok(Async::Ready(self.t.take().expect("cannot poll Ok twice")))
    }
}
