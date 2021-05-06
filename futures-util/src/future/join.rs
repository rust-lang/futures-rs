use crate::future::{assert_future, maybe_done, MaybeDone};
use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`join`](super::FutureExt::join) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Join<Fut1, Fut2> where Fut1: Future, Fut2: Future {
        #[pin] fut1: MaybeDone<Fut1>,
        #[pin] fut2: MaybeDone<Fut2>,
    }
}

impl<Fut1: Future, Fut2: Future> Join<Fut1, Fut2> {
    pub(crate) fn new(fut1: Fut1, fut2: Fut2) -> Self {
        Self { fut1: maybe_done(fut1), fut2: maybe_done(fut2) }
    }
}

impl<Fut1, Fut2> fmt::Debug for Join<Fut1, Fut2>
where
    Fut1: Future + fmt::Debug,
    Fut1::Output: fmt::Debug,
    Fut2: Future + fmt::Debug,
    Fut2::Output: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Join").field("fut1", &self.fut1).field("fut2", &self.fut2).finish()
    }
}

impl<Fut1: Future, Fut2: Future> Future for Join<Fut1, Fut2> {
    type Output = (Fut1::Output, Fut2::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut all_done = true;
        let mut futures = self.project();

        all_done &= futures.fut1.as_mut().poll(cx).is_ready();
        all_done &= futures.fut2.as_mut().poll(cx).is_ready();

        if all_done {
            Poll::Ready((futures.fut1.take_output().unwrap(), futures.fut2.take_output().unwrap()))
        } else {
            Poll::Pending
        }
    }
}

impl<Fut1: FusedFuture, Fut2: FusedFuture> FusedFuture for Join<Fut1, Fut2> {
    fn is_terminated(&self) -> bool {
        self.fut1.is_terminated() && self.fut2.is_terminated()
    }
}

/// Joins the result of two futures, waiting for them both to complete.
///
/// This function will return a new future which awaits both futures to
/// complete. The returned future will finish with a tuple of both results.
///
/// Note that this function consumes the passed futures and returns a
/// wrapped version of it.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = async { 1 };
/// let b = async { 2 };
/// let pair = future::join(a, b);
///
/// assert_eq!(pair.await, (1, 2));
/// # });
/// ```
pub fn join<Fut1, Fut2>(future1: Fut1, future2: Fut2) -> Join<Fut1, Fut2>
where
    Fut1: Future,
    Fut2: Future,
{
    assert_future::<(Fut1::Output, Fut2::Output), _>(Join::new(future1, future2))
}
