//! Definition of the `Option` (optional step) combinator

use futures_core::future::Future;
use futures_core::task::{self, Poll};
use core::mem::PinMut;

/// A future representing a value which may or may not be present.
///
/// Created by the `From` implementation for `std::option::Option`.
///
/// ```
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::future::OptionFuture;
///
/// let fut: OptionFuture<_> = Some(123).into();
/// ```
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct OptionFuture<F> {
    option: Option<F>,
}

impl<F> OptionFuture<F> {
    unsafe_pinned!(option: Option<F>);
}

impl<F: Future> Future for OptionFuture<F> {
    type Output = Option<F::Output>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context
    ) -> Poll<Self::Output> {
        match self.option().as_pin_mut() {
            Some(x) => x.poll(cx).map(Some),
            None => Poll::Ready(None),
        }
    }
}

impl<T> From<Option<T>> for OptionFuture<T> {
    fn from(option: Option<T>) -> Self {
        OptionFuture { option }
    }
}
