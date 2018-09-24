#![allow(non_snake_case)]

use crate::future::{MaybeDone, maybe_done};
use crate::try_future::{TryFutureExt, IntoFuture};
use core::fmt;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

macro_rules! generate {
    ($(
        $(#[$doc:meta])*
        ($Join:ident, <Fut1, $($Fut:ident),*>),
    )*) => ($(
        $(#[$doc])*
        #[must_use = "futures do nothing unless polled"]
        pub struct $Join<Fut1: TryFuture, $($Fut: TryFuture),*> {
            Fut1: MaybeDone<IntoFuture<Fut1>>,
            $($Fut: MaybeDone<IntoFuture<$Fut>>,)*
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
            fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.debug_struct(stringify!($Join))
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
            pub(super) fn new(Fut1: Fut1, $($Fut: $Fut),*) -> $Join<Fut1, $($Fut),*> {
                $Join {
                    Fut1: maybe_done(TryFutureExt::into_future(Fut1)),
                    $($Fut: maybe_done(TryFutureExt::into_future($Fut))),*
                }
            }

            unsafe_pinned!(Fut1: MaybeDone<IntoFuture<Fut1>>);
            $(
                unsafe_pinned!($Fut: MaybeDone<IntoFuture<$Fut>>);
            )*
        }

        impl<Fut1, $($Fut),*> Future for $Join<Fut1, $($Fut),*>
        where
            Fut1: TryFuture,
            $(
                $Fut: TryFuture<Error=Fut1::Error>
            ),*
        {
            type Output = Result<(Fut1::Ok, $($Fut::Ok),*), Fut1::Error>;

            #[allow(clippy::useless_let_if_seq)]
            fn poll(
                mut self: Pin<&mut Self>, lw: &LocalWaker
            ) -> Poll<Self::Output> {
                let mut all_done = true;
                if self.Fut1().poll(cx).is_pending() {
                    all_done = false;
                } else if self.Fut1().output_mut().unwrap().is_err() {
                    return Poll::Ready(Err(
                        self.Fut1().take_output().unwrap().err().unwrap()));
                }
                $(
                    if self.$Fut().poll(cx).is_pending() {
                        all_done = false;
                    } else if self.$Fut().output_mut().unwrap().is_err() {
                        return Poll::Ready(Err(
                            self.$Fut().take_output().unwrap().err().unwrap()));
                    }
                )*

                if all_done {
                    Poll::Ready(Ok((
                        self.Fut1().take_output().unwrap().ok().unwrap(),
                        $(
                            self.$Fut().take_output().unwrap().ok().unwrap()
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
    /// Future for the `try_join` combinator, waiting for two futures to
    /// complete or for one to error.
    ///
    /// This is created by the `TryFuture::try_join` method.
    (TryJoin, <Fut1, Fut2>),

    /// Future for the `try_join3` combinator, waiting for three futures to
    /// complete or for one to error.
    ///
    /// This is created by the `TryFuture::try_join3` method.
    (TryJoin3, <Fut1, Fut2, Fut3>),

    /// Future for the `try_join4` combinator, waiting for four futures to
    /// complete or for one to error.
    ///
    /// This is created by the `TryFuture::try_join4` method.
    (TryJoin4, <Fut1, Fut2, Fut3, Fut4>),

    /// Future for the `try_join5` combinator, waiting for five futures to
    /// complete or for one to error.
    ///
    /// This is created by the `TryFuture::try_join5` method.
    (TryJoin5, <Fut1, Fut2, Fut3, Fut4, Fut5>),
}
