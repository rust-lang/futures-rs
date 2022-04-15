use crate::future::assert_future;
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`join`](super::FutureExt::join_with) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct JoinWith<F: Future, T> {
        #[pin]
        future: F,
        data: T,
    }
}

impl<Fut: Future, T> JoinWith<Fut, T> {
    pub(crate) fn new(future: Fut, data: T) -> Self {
        JoinWith { future, data }
    }
}

impl<Fut, T> fmt::Debug for JoinWith<Fut, T>
where
    Fut: Future + fmt::Debug,
    Fut::Output: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinWith").field("future", &self.future).field("data", &self.data).finish()
    }
}

impl<F: Future, T: Copy> Future for JoinWith<F, T> {
    type Output = (F::Output, T);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let future = self.as_mut().project().future;
        match future.poll(cx) {
            Poll::Ready(v) => Poll::Ready((v, self.data)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Joins a future with a value.
///
/// This function will return a new future which awaits the future to
/// complete. The returned future will finish with a tuple of the result
/// of the future and the provided value.
///
/// Note that this function consumes the passed future and returns a
/// wrapped version of it.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let f = async { 1 };
/// let joined = future::join_with(f, 2);
///
/// assert_eq!(joined.await, (1, 2));
/// # });
/// ```
pub fn join_with<Fut, T>(future: Fut, data: T) -> JoinWith<Fut, T>
where
    Fut: Future,
    T: Copy,
{
    assert_future::<(Fut::Output, T), _>(JoinWith::new(future, data))
}
