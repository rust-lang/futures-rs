//! Definition of the `Option` (optional step) combinator

use {Future, IntoFuture, Poll, Async};
use task;

/// A future representing a value which may or may not be present.
///
/// Created by the `IntoFuture` implementation for `std::option::Option`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct FutureOption<T> {
    inner: Option<T>,
}

impl<T> IntoFuture for Option<T> where T: IntoFuture {
    type Future = FutureOption<T::Future>;
    type Item = Option<T::Item>;
    type Error = T::Error;

    fn into_future(self) -> FutureOption<T::Future> {
        FutureOption { inner: self.map(IntoFuture::into_future) }
    }
}

impl<F, T, E> Future for FutureOption<F> where F: Future<Item=T, Error=E> {
    type Item = Option<T>;
    type Error = E;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Option<T>, E> {
        match self.inner {
            None => Ok(Async::Ready(None)),
            Some(ref mut x) => x.poll(cx).map(|x| x.map(Some)),
        }
    }
}

impl<T> From<Option<T>> for FutureOption<T> {
    fn from(o: Option<T>) -> Self {
        FutureOption { inner: o }
    }
}
