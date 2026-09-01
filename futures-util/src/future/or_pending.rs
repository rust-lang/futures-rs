//! Definition of [`OrPending`] future wrapper.

use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// A wrapper for a [`Future`] which may or may not be present.
    ///
    /// If the inner future is present, this wrapper will resolve when it does.
    /// Otherwise this wrapper will never resolve (`Future::poll` will always
    /// return [`Poll::Pending`]). Created by the [`From`] implementation for
    /// [`Option`](std::option::Option).
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::OptionFuture;
    ///
    /// let mut a: OptionFuture<_> = Some(async { 123 }).into();
    /// assert_eq!(a.await, Some(123));
    ///
    /// a = None.into();
    /// assert_eq!(a.await, None);
    /// # });
    /// ```
    #[derive(Debug, Clone)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct OrPending<F> {
        #[pin]
        inner: Option<F>,
    }
}

impl<F> OrPending<F> {
    /// Gets a mutable reference to the inner future (if any).
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Option<Pin<&mut F>> {
        self.project().inner.as_pin_mut()
    }

    /// Drops the inner future.
    ///
    /// After this, all calls to [`Future::poll`] will return [`Poll::Pending`].
    pub fn reset(self: Pin<&mut Self>) {
        self.project().inner.set(None);
    }
}

impl<F> Default for OrPending<F> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<F: Future> Future for OrPending<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        match this.inner.as_mut().as_pin_mut() {
            None => Poll::Pending,
            Some(x) => match x.poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(t) => {
                    this.inner.set(None);
                    Poll::Ready(t)
                }
            },
        }
    }
}

impl<F: FusedFuture> FusedFuture for OrPending<F> {
    fn is_terminated(&self) -> bool {
        match &self.inner {
            Some(x) => x.is_terminated(),
            None => true,
        }
    }
}

impl<T> From<Option<T>> for OrPending<T> {
    fn from(option: Option<T>) -> Self {
        Self { inner: option }
    }
}
