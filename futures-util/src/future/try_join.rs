#![allow(non_snake_case)]

use crate::future::{assert_future, try_maybe_done, TryMaybeDone};
use core::fmt;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

macro_rules! generate {
    ($(
        $(#[$doc:meta])*
        ($Join:ident, <Fut1, $($Fut:ident),*>),
    )*) => ($(
        pin_project! {
            $(#[$doc])*
            #[must_use = "futures do nothing unless you `.await` or poll them"]
            pub struct $Join<Fut1: TryFuture, $($Fut: TryFuture),*> {
                #[pin] Fut1: TryMaybeDone<Fut1>,
                $(#[pin] $Fut: TryMaybeDone<$Fut>,)*
            }
        }

        impl<Fut1, $($Fut),*> fmt::Debug for $Join<Fut1, $($Fut),*>
        where
            Fut1: TryFuture + fmt::Debug,
            Fut1::Ok: fmt::Debug,
            Fut1::Error: fmt::Debug,
            $(
                $Fut: TryFuture + fmt::Debug,
                $Fut::Ok: fmt::Debug,
                $Fut::Error: fmt::Debug,
            )*
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Join))
                    .field("Fut1", &self.Fut1)
                    $(.field(stringify!($Fut), &self.$Fut))*
                    .finish()
            }
        }

        impl<Fut1, $($Fut),*> $Join<Fut1, $($Fut),*>
        where
            Fut1: TryFuture,
            $(
                $Fut: TryFuture<Error=Fut1::Error>
            ),*
        {
            fn new(Fut1: Fut1, $($Fut: $Fut),*) -> Self {
                Self {
                    Fut1: try_maybe_done(Fut1),
                    $($Fut: try_maybe_done($Fut)),*
                }
            }
        }

        impl<Fut1, $($Fut),*> Future for $Join<Fut1, $($Fut),*>
        where
            Fut1: TryFuture,
            $(
                $Fut: TryFuture<Error=Fut1::Error>
            ),*
        {
            type Output = Result<(Fut1::Ok, $($Fut::Ok),*), Fut1::Error>;

            fn poll(
                self: Pin<&mut Self>, cx: &mut Context<'_>
            ) -> Poll<Self::Output> {
                let mut all_done = true;
                let mut futures = self.project();
                all_done &= futures.Fut1.as_mut().poll(cx)?.is_ready();
                $(
                    all_done &= futures.$Fut.as_mut().poll(cx)?.is_ready();
                )*

                if all_done {
                    Poll::Ready(Ok((
                        futures.Fut1.take_output().unwrap(),
                        $(
                            futures.$Fut.take_output().unwrap()
                        ),*
                    )))
                } else {
                    Poll::Pending
                }
            }
        }
    )*)
}

generate! {
    /// Future for the [`try_join`](try_join()) function.
    (TryJoin, <Fut1, Fut2>),
}

/// Joins the result of two futures, waiting for them both to complete or
/// for one to produce an error.
///
/// This function will return a new future which awaits both futures to
/// complete. If successful, the returned future will finish with a tuple of
/// both results. If unsuccessful, it will complete with the first error
/// encountered.
///
/// Note that this function consumes the passed futures and returns a
/// wrapped version of it.
///
/// # Examples
///
/// When used on multiple futures that return [`Ok`], `try_join` will return
/// [`Ok`] of a tuple of the values:
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::ready(Ok::<i32, i32>(1));
/// let b = future::ready(Ok::<i32, i32>(2));
/// let pair = future::try_join(a, b);
///
/// assert_eq!(pair.await, Ok((1, 2)));
/// # });
/// ```
///
/// If one of the futures resolves to an error, `try_join` will return
/// that error:
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::ready(Ok::<i32, i32>(1));
/// let b = future::ready(Err::<i32, i32>(2));
/// let pair = future::try_join(a, b);
///
/// assert_eq!(pair.await, Err(2));
/// # });
/// ```
pub fn try_join<Fut1, Fut2>(future1: Fut1, future2: Fut2) -> TryJoin<Fut1, Fut2>
where
    Fut1: TryFuture,
    Fut2: TryFuture<Error = Fut1::Error>,
{
    assert_future::<Result<(Fut1::Ok, Fut2::Ok), Fut1::Error>, _>(TryJoin::new(future1, future2))
}
