//! Definition of the `Option` (optional step) combinator

use {Future, Poll};
use task;
use core::mem::PinMut;

/// A future representing a value which may or may not be present.
///
/// Created by the `IntoFuture` implementation for `std::option::Option`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct FutureOption<F> {
    option: Option<F>,
}

impl<F> FutureOption<F> {
    unsafe_pinned!(option -> Option<F>);
}

impl<F: Future> Future for FutureOption<F> {
    type Output = Option<F::Output>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        match self.option().as_pin_mut() {
            Some(x) => x.poll(cx).map(Some),
            None => Poll::Ready(None),
        }
    }
}

impl<T> From<Option<T>> for FutureOption<T> {
    fn from(option: Option<T>) -> Self {
        FutureOption { option }
    }
}
