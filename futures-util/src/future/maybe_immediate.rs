use super::{assert_future, MaybeDone};
use core::fmt::Debug;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::future::Future;
use futures_core::FusedFuture;
use pin_project_lite::pin_project;

pin_project! {
    /// A future that may wrap another future or immediately return a value.
    ///
    /// This is created by the [`maybe_immediate()`] function and via
    /// [`FutureExt::maybe_immediate`].
    pub struct MaybeImmediate<Fut: Future> {
        #[pin]
        inner: MaybeDone<Fut>,
    }
}

impl<Fut: Future> MaybeImmediate<Fut> {
    pub(super) fn new(future: Fut) -> Self {
        let inner = MaybeDone::Future(future);
        Self { inner }
    }

    fn new_immediate(value: Fut::Output) -> Self {
        let inner = MaybeDone::Done(value);
        Self { inner }
    }
}

/// Wraps a value into a `MaybeImmediate`
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
/// use futures::future::FutureExt;
/// use futures::Future;
///
/// fn immediately_or_later(now: bool) -> impl Future<Output = &'static str> {
///     if now {
///         future::maybe_immediate("now")
///     } else {
///         async {
///             "later"
///         }.maybe_immediate()
///     }
/// }
///
/// let immediate_future = immediately_or_later(true);
/// assert_eq!("now", immediate_future.await);
/// let later_future = immediately_or_later(false);
/// assert_eq!("later", later_future.await);
/// # });
/// ```
pub fn maybe_immediate<Fut: Future>(value: Fut::Output) -> MaybeImmediate<Fut> {
    assert_future::<Fut::Output, _>(MaybeImmediate::new_immediate(value))
}

// `#[derive(Debug)]` fails to compile with
// "error[E0277]: `<Fut as Future>::Output` doesn't implement `Debug`"
impl<Fut: Future> Debug for MaybeImmediate<Fut>
where
    MaybeDone<Fut>: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MaybeImmediate").field("inner", &self.inner).finish()
    }
}

impl<Fut: Future> FusedFuture for MaybeImmediate<Fut> {
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<Fut: Future> Future for MaybeImmediate<Fut> {
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.project().inner;
        match inner.as_mut().poll(cx) {
            Poll::Ready(()) => {
                Poll::Ready(inner.take_output().expect("MaybeImmediate polled after termination"))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
