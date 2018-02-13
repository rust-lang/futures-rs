//! Definition of the `Option` (optional step) combinator

use {Future, FutureMove, IntoFuture, Poll, Async};

use core::option;

/// A future representing a value which may or may not be present.
///
/// Created by the `IntoFuture` implementation for `std::option::Option`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Option<T> {
    inner: option::Option<T>,
}

impl<T> IntoFuture for option::Option<T> where T: IntoFuture {
    type Future = Option<T::Future>;
    type Item = option::Option<T::Item>;
    type Error = T::Error;

    fn into_future(self) -> Option<T::Future> {
        Option { inner: self.map(IntoFuture::into_future) }
    }
}

impl<F, T, E> Future for Option<F> where F: Future<Item=T, Error=E> {
    type Item = option::Option<T>;
    type Error = E;

    unsafe fn poll_unsafe(&mut self) -> Poll<option::Option<T>, E> {
        match self.inner {
            None => Ok(Async::Ready(None)),
            Some(ref mut x) => x.poll_unsafe().map(|x| x.map(Some)),
        }
    }
}

impl<F, T, E> FutureMove for Option<F> where F: FutureMove<Item=T, Error=E> {
    fn poll_move(&mut self) -> Poll<option::Option<T>, E> {
        match self.inner {
            None => Ok(Async::Ready(None)),
            Some(ref mut x) => x.poll_move().map(|x| x.map(Some)),
        }
    }
}
