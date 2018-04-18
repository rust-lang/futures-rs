//! Definition of the `Option` (optional step) combinator

use {Future, Poll};
use task;
use core::mem::Pin;

/// A future representing a value which may or may not be present.
///
/// Created by the `IntoFuture` implementation for `std::option::Option`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct FutureOption<T> {
    inner: Option<T>,
}

impl<F: Future> Future for FutureOption<F> {
    type Output = Option<F::Output>;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe {
            match Pin::get_mut(&mut self).inner {
                None => Poll::Ready(None),
                Some(ref mut x) => Pin::new_unchecked(x).poll(cx).map(Some),
            }
        }
    }
}

impl<T> From<Option<T>> for FutureOption<T> {
    fn from(o: Option<T>) -> Self {
        FutureOption { inner: o }
    }
}
