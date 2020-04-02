use crate::future::Either;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_utils::unsafe_pinned;

/// Future for the [`first()`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug, Clone, Default)]
pub struct First<F1, F2> {
    future1: F1,
    future2: F2,
}

impl<F1, F2> First<F1, F2> {
    unsafe_pinned!(future1: F1);
    unsafe_pinned!(future2: F2);
}

impl<F1: Future, F2: Future> Future for First<F1, F2> {
    type Output = Either<F1::Output, F2::Output>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.as_mut().future1().poll(cx) {
            Poll::Ready(out) => Poll::Ready(Either::Left(out)),
            Poll::Pending => match self.future2().poll(cx) {
                Poll::Ready(out) => Poll::Ready(Either::Right(out)),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl<F1: FusedFuture, F2: FusedFuture> FusedFuture for First<F1, F2> {
    #[inline]
    fn is_terminated(&self) -> bool {
        self.future1.is_terminated() || self.future2.is_terminated()
    }
}

/// Waits for either one of two differently-typed futures to complete.
///
/// This function will return a new future which awaits for either one of both
/// futures to complete. The returned future will finish with the value of
/// whichever future finishes first.
///
/// The future will discard the future that didn't complete; see `select` for
/// a future that will instead return the incomplete future.
///
/// Note that this function consumes the receiving futures and returns a
/// wrapped version of them.
///
/// Also note that if both this and the second future have the same
/// output type you can use the `Either::factor_first` method to
/// conveniently extract out the value at the end.
pub fn first<F1, F2>(future1: F1, future2: F2) -> First<F1, F2> {
    First { future1, future2 }
}

#[test]
fn test_first() {
    use crate::future::{pending, ready, FutureExt};
    use crate::task::noop_waker_ref;

    let mut context = Context::from_waker(noop_waker_ref());

    assert_eq!(
        first(ready(10), ready(20)).poll_unpin(&mut context),
        Poll::Ready(Either::Left(10))
    );
    assert_eq!(
        first(ready(10), pending::<()>()).poll_unpin(&mut context),
        Poll::Ready(Either::Left(10))
    );
    assert_eq!(
        first(pending::<()>(), ready(20)).poll_unpin(&mut context),
        Poll::Ready(Either::Right(20))
    );
    assert_eq!(
        first(pending::<()>(), pending::<()>()).poll_unpin(&mut context),
        Poll::Pending
    );
}
