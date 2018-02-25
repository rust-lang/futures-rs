//! Definition of the `Option` (optional step) combinator

use {Future, IntoFuture, Poll, Async};
use core::option;

/// An optional future.
///
/// Created by `Option::into_future`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Option<F> {
    inner: option::Option<F>,
}

impl<F> IntoFuture for option::Option<F> where F: Future {
    type Future = Option<F>;
    type Item = option::Option<F::Item>;
    type Error = F::Error;

    fn into_future(self) -> Self::Future {
        Option { inner: self }
    }
}

impl<F> Future for Option<F> where F: Future {
    type Item = option::Option<F::Item>;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner {
            None => Ok(Async::Ready(None)),
            Some(ref mut x) => x.poll().map(|x| x.map(Some)),
        }
    }
}
