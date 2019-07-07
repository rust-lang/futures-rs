//! Definition of the `Option` (optional step) combinator

use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// A future representing a value which may or may not be present.
///
/// Created by the [`From`] implementation for [`Option`](std::option::Option).
///
/// # Examples
///
/// ```
/// #![feature(async_await)]
/// # futures::executor::block_on(async {
/// use futures::future::{self, OptionFuture};
///
/// let mut a: OptionFuture<_> = Some(future::ready(123)).into();
/// assert_eq!(a.await, Some(123));
///
/// a = None.into();
/// assert_eq!(a.await, None);
/// # });
/// ```
#[unsafe_project(Unpin)]
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct OptionFuture<F> {
    #[pin]
    option: Option<F>,
}

impl<F: Future> Future for OptionFuture<F> {
    type Output = Option<F::Output>;

    #[pin_project(self)]
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        match self.option.as_pin_mut() {
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
