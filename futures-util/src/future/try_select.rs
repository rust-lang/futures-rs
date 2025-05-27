use super::{Biased, Fair, IsBiased};
use crate::future::{Either, TryFutureExt};
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{Context, Poll};

/// Future for the [`try_select()`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct TrySelect<A, B, BIASED = Fair> {
    inner: Option<(A, B)>,
    _phantom: PhantomData<BIASED>,
}

impl<A: Unpin, B: Unpin, BIASED> Unpin for TrySelect<A, B, BIASED> {}

type EitherOk<A, B> = Either<(<A as TryFuture>::Ok, B), (<B as TryFuture>::Ok, A)>;
type EitherErr<A, B> = Either<(<A as TryFuture>::Error, B), (<B as TryFuture>::Error, A)>;

/// Waits for either one of two differently-typed futures to complete.
///
/// This function will return a new future which awaits for either one of both
/// futures to complete. The returned future will finish with both the value
/// resolved and a future representing the completion of the other work.
///
/// Note that this function consumes the receiving futures and returns a
/// wrapped version of them.
///
/// If both futures are ready when this is polled, the winner will be pseudo-randomly
/// selected, unless the `std` feature is disabled. If the std feature is disabled,
/// the first argument will always win.
///
/// Also note that if both this and the second future have the same
/// success/error type you can use the `Either::factor_first` method to
/// conveniently extract out the value at the end.
///
/// # Examples
///
/// ```
/// use futures::future::{self, Either, Future, FutureExt, TryFuture, TryFutureExt};
///
/// // A poor-man's try_join implemented on top of select
///
/// fn try_join<A, B, E>(a: A, b: B) -> impl TryFuture<Ok=(A::Ok, B::Ok), Error=E>
///      where A: TryFuture<Error = E> + Unpin + 'static,
///            B: TryFuture<Error = E> + Unpin + 'static,
///            E: 'static,
/// {
///     future::try_select(a, b).then(|res| -> Box<dyn Future<Output = Result<_, _>> + Unpin> {
///         match res {
///             Ok(Either::Left((x, b))) => Box::new(b.map_ok(move |y| (x, y))),
///             Ok(Either::Right((y, a))) => Box::new(a.map_ok(move |x| (x, y))),
///             Err(Either::Left((e, _))) => Box::new(future::err(e)),
///             Err(Either::Right((e, _))) => Box::new(future::err(e)),
///         }
///     })
/// }
/// ```
pub fn try_select<A, B>(future1: A, future2: B) -> TrySelect<A, B, Fair>
where
    A: TryFuture + Unpin,
    B: TryFuture + Unpin,
{
    super::assert_future::<Result<EitherOk<A, B>, EitherErr<A, B>>, _>(TrySelect {
        inner: Some((future1, future2)),
        _phantom: PhantomData,
    })
}

/// Waits for either one of two differently-typed futures to complete, giving preferential treatment to the first one.
///
/// This function will return a new future which awaits for either one of both
/// futures to complete. The returned future will finish with both the value
/// resolved and a future representing the completion of the other work.
///
/// Note that this function consumes the receiving futures and returns a
/// wrapped version of them.
///
/// If both futures are ready when this is polled, the winner will always be the first one.
///
/// Also note that if both this and the second future have the same
/// success/error type you can use the `Either::factor_first` method to
/// conveniently extract out the value at the end.
///
/// # Examples
///
/// ```
/// use futures::future::{self, Either, Future, FutureExt, TryFuture, TryFutureExt};
///
/// // A poor-man's try_join implemented on top of select
///
/// fn try_join<A, B, E>(a: A, b: B) -> impl TryFuture<Ok=(A::Ok, B::Ok), Error=E>
///      where A: TryFuture<Error = E> + Unpin + 'static,
///            B: TryFuture<Error = E> + Unpin + 'static,
///            E: 'static,
/// {
///     future::try_select_biased(a, b).then(|res| -> Box<dyn Future<Output = Result<_, _>> + Unpin> {
///         match res {
///             Ok(Either::Left((x, b))) => Box::new(b.map_ok(move |y| (x, y))),
///             Ok(Either::Right((y, a))) => Box::new(a.map_ok(move |x| (x, y))),
///             Err(Either::Left((e, _))) => Box::new(future::err(e)),
///             Err(Either::Right((e, _))) => Box::new(future::err(e)),
///         }
///     })
/// }
/// ```
pub fn try_select_biased<A, B>(future1: A, future2: B) -> TrySelect<A, B, Biased>
where
    A: TryFuture + Unpin,
    B: TryFuture + Unpin,
{
    super::assert_future::<Result<EitherOk<A, B>, EitherErr<A, B>>, _>(TrySelect {
        inner: Some((future1, future2)),
        _phantom: PhantomData,
    })
}

impl<A: Unpin, B: Unpin, BIASED> Future for TrySelect<A, B, BIASED>
where
    A: TryFuture,
    B: TryFuture,
    BIASED: IsBiased,
{
    type Output = Result<EitherOk<A, B>, EitherErr<A, B>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut a, mut b) = self.inner.take().expect("cannot poll Select twice");
        macro_rules! poll_wrap {
            ($poll_first:expr, $poll_second:expr, $wrap_first:expr, $wrap_second:expr) => {
                match $poll_first.try_poll_unpin(cx) {
                    Poll::Ready(Err(x)) => Poll::Ready(Err($wrap_first((x, $poll_second)))),
                    Poll::Ready(Ok(x)) => Poll::Ready(Ok($wrap_first((x, $poll_second)))),
                    Poll::Pending => match $poll_second.try_poll_unpin(cx) {
                        Poll::Ready(Err(x)) => Poll::Ready(Err($wrap_second((x, $poll_first)))),
                        Poll::Ready(Ok(x)) => Poll::Ready(Ok($wrap_second((x, $poll_first)))),
                        Poll::Pending => {
                            self.inner = Some((a, b));
                            Poll::Pending
                        }
                    },
                }
            };
        }

        #[cfg(feature = "std")]
        {
            if BIASED::IS_BIASED || crate::gen_index(2) == 0 {
                poll_wrap!(a, b, Either::Left, Either::Right)
            } else {
                poll_wrap!(b, a, Either::Right, Either::Left)
            }
        }

        #[cfg(not(feature = "std"))]
        {
            poll_wrap!(a, b, Either::Left, Either::Right)
        }
    }
}
