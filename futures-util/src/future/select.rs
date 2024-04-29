use super::{assert_future, Biased, Fair, IsBiased};
use crate::future::{Either, FutureExt};
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};

/// Future for the [`select()`] function.
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct Select<A, B, BIASED = Fair> {
    inner: Option<(A, B)>,
    _phantom: PhantomData<BIASED>,
}

impl<A: Unpin, B: Unpin, BIASED> Unpin for Select<A, B, BIASED> {}

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
/// selected, unless the std feature is not enabled. If std is not enabled, the first
/// argument will always win.
///
/// Also note that if both this and the second future have the same
/// output type you can use the `Either::factor_first` method to
/// conveniently extract out the value at the end.
///
/// # Examples
///
/// A simple example
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::{
///     pin_mut,
///     future::Either,
///     future::self,
/// };
///
/// // These two futures have different types even though their outputs have the same type.
/// let future1 = async {
///     future::pending::<()>().await; // will never finish
///     1
/// };
/// let future2 = async {
///     future::ready(2).await
/// };
///
/// // 'select' requires Future + Unpin bounds
/// pin_mut!(future1);
/// pin_mut!(future2);
///
/// let value = match future::select(future1, future2).await {
///     Either::Left((value1, _)) => value1,  // `value1` is resolved from `future1`
///                                           // `_` represents `future2`
///     Either::Right((value2, _)) => value2, // `value2` is resolved from `future2`
///                                           // `_` represents `future1`
/// };
///
/// assert!(value == 2);
/// # });
/// ```
///
/// A more complex example
///
/// ```
/// use futures::future::{self, Either, Future, FutureExt};
///
/// // A poor-man's join implemented on top of select
///
/// fn join<A, B>(a: A, b: B) -> impl Future<Output=(A::Output, B::Output)>
///     where A: Future + Unpin,
///           B: Future + Unpin,
/// {
///     future::select(a, b).then(|either| {
///         match either {
///             Either::Left((x, b)) => b.map(move |y| (x, y)).left_future(),
///             Either::Right((y, a)) => a.map(move |x| (x, y)).right_future(),
///         }
///     })
/// }
/// ```
pub fn select<A, B>(future1: A, future2: B) -> Select<A, B, Fair>
where
    A: Future + Unpin,
    B: Future + Unpin,
{
    assert_future::<Either<(A::Output, B), (B::Output, A)>, _>(Select {
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
/// If both futures are ready when this is polled, the winner will always be the first argument.
///
/// Also note that if both this and the second future have the same
/// output type you can use the `Either::factor_first` method to
/// conveniently extract out the value at the end.
///
/// # Examples
///
/// A simple example
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::{
///     pin_mut,
///     future::Either,
///     future::self,
/// };
///
/// // These two futures have different types even though their outputs have the same type.
/// let future1 = async {
///     future::pending::<()>().await; // will never finish
///     1
/// };
/// let future2 = async {
///     future::ready(2).await
/// };
///
/// // 'select_biased' requires Future + Unpin bounds
/// pin_mut!(future1);
/// pin_mut!(future2);
///
/// let value = match future::select_biased(future1, future2).await {
///     Either::Left((value1, _)) => value1,  // `value1` is resolved from `future1`
///                                           // `_` represents `future2`
///     Either::Right((value2, _)) => value2, // `value2` is resolved from `future2`
///                                           // `_` represents `future1`
/// };
///
/// assert!(value == 2);
/// # });
/// ```
///
/// A more complex example
///
/// ```
/// use futures::future::{self, Either, Future, FutureExt};
///
/// // A poor-man's join implemented on top of select
///
/// fn join<A, B>(a: A, b: B) -> impl Future<Output=(A::Output, B::Output)>
///     where A: Future + Unpin,
///           B: Future + Unpin,
/// {
///     future::select_biased(a, b).then(|either| {
///         match either {
///             Either::Left((x, b)) => b.map(move |y| (x, y)).left_future(),
///             Either::Right((y, a)) => a.map(move |x| (x, y)).right_future(),
///         }
///     })
/// }
/// ```
pub fn select_biased<A, B>(future1: A, future2: B) -> Select<A, B, Biased>
where
    A: Future + Unpin,
    B: Future + Unpin,
{
    assert_future::<Either<(A::Output, B), (B::Output, A)>, _>(Select {
        inner: Some((future1, future2)),
        _phantom: PhantomData,
    })
}

impl<A, B, BIASED> Future for Select<A, B, BIASED>
where
    A: Future + Unpin,
    B: Future + Unpin,
    BIASED: IsBiased,
{
    type Output = Either<(A::Output, B), (B::Output, A)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        /// When compiled with `-C opt-level=z`, this function will help the compiler eliminate the `None` branch, where
        /// `Option::unwrap` does not.
        #[inline(always)]
        fn unwrap_option<T>(value: Option<T>) -> T {
            match value {
                None => unreachable!(),
                Some(value) => value,
            }
        }

        let (a, b) = self.inner.as_mut().expect("cannot poll Select twice");

        macro_rules! poll_wrap {
            ($to_poll:expr, $unpolled:expr, $wrap:expr) => {
                if let Poll::Ready(val) = $to_poll.poll_unpin(cx) {
                    return Poll::Ready($wrap((val, $unpolled)));
                }
            };
        }

        #[cfg(feature = "std")]
        if BIASED::IS_BIASED || crate::gen_index(2) == 0 {
            poll_wrap!(a, unwrap_option(self.inner.take()).1, Either::Left);
            poll_wrap!(b, unwrap_option(self.inner.take()).0, Either::Right);
        } else {
            poll_wrap!(b, unwrap_option(self.inner.take()).0, Either::Right);
            poll_wrap!(a, unwrap_option(self.inner.take()).1, Either::Left);
        }

        #[cfg(not(feature = "std"))]
        {
            poll_wrap!(a, unwrap_option(self.inner.take()).1, Either::Left);
            poll_wrap!(b, unwrap_option(self.inner.take()).0, Either::Right);
        }
        Poll::Pending
    }
}

impl<A, B, BIASED> FusedFuture for Select<A, B, BIASED>
where
    A: Future + Unpin,
    B: Future + Unpin,
    BIASED: IsBiased,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}
