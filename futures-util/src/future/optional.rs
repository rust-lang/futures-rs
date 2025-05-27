//! Definition of the `Optional` (optional step) combinator

use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// A future representing an operation which may or may not exist.
    ///
    /// Created by the [`From`] implementation for [`Option`](std::option::Option).
    ///
    /// A future made from `None` will never resolve. If you wish
    /// it resolved with `None`, use [`OptionFuture`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::{OptionalFuture, ready};
    /// use futures::select_biased;
    ///
    /// let mut a: OptionalFuture<_> = Some(ready(123)).into();
    /// let mut b = ready(());
    /// assert_eq!(
    ///     select_biased! {
    ///         _ = a => 1,
    ///         _ = b => 2,
    ///     },
    ///     1
    /// );
    ///
    /// a = None.into();
    /// b = ready(());
    /// assert_eq!(
    ///     select_biased! {
    ///         _ = a => 1,
    ///         _ = b => 2,
    ///     },
    ///     2
    /// );
    /// # });
    /// ```
    #[derive(Debug, Clone)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct OptionalFuture<F> {
        #[pin]
        inner: Option<F>,
    }
}

impl<F> Default for OptionalFuture<F> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<F: Future> Future for OptionalFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().inner.as_pin_mut() {
            Some(x) => x.poll(cx),
            None => Poll::Pending,
        }
    }
}

impl<F: FusedFuture> FusedFuture for OptionalFuture<F> {
    fn is_terminated(&self) -> bool {
        match &self.inner {
            Some(x) => x.is_terminated(),
            None => true,
        }
    }
}

impl<T> From<Option<T>> for OptionalFuture<T> {
    fn from(option: Option<T>) -> Self {
        Self { inner: option }
    }
}
