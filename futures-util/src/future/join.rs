#![allow(non_snake_case)]

use crate::future::{MaybeDone, maybe_done};
use core::fmt;
use core::pin::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

macro_rules! generate {
    ($(
        $(#[$doc:meta])*
        ($Join:ident, <$($Fut:ident),*>),
    )*) => ($(
        $(#[$doc])*
        #[must_use = "futures do nothing unless polled"]
        pub struct $Join<$($Fut: Future),*> {
            $($Fut: MaybeDone<$Fut>,)*
        }

        impl<$($Fut),*> fmt::Debug for $Join<$($Fut),*>
        where
            $(
                $Fut: Future + fmt::Debug,
                $Fut::Output: fmt::Debug,
            )*
        {
            fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.debug_struct(stringify!($Join))
                    $(.field(stringify!($Fut), &self.$Fut))*
                    .finish()
            }
        }

        impl<$($Fut: Future),*> $Join<$($Fut),*> {
            pub(super) fn new($($Fut: $Fut),*) -> $Join<$($Fut),*> {
                $Join {
                    $($Fut: maybe_done($Fut)),*
                }
            }
            $(
                unsafe_pinned!($Fut: MaybeDone<$Fut>);
            )*
        }

        impl<$($Fut: Future),*> Future for $Join<$($Fut),*> {
            type Output = ($($Fut::Output),*);

            #[allow(useless_let_if_seq)]
            fn poll(
                mut self: PinMut<Self>, cx: &mut task::Context
            ) -> Poll<Self::Output> {
                let mut all_done = true;
                $(
                    if self.$Fut().poll(cx).is_pending() {
                        all_done = false;
                    }
                )*

                if all_done {
                    Poll::Ready(($(self.$Fut().take_output().unwrap()), *))
                } else {
                    Poll::Pending
                }
            }
        }
    )*)
}

generate! {
    /// Future for the `join` combinator, waiting for two futures to
    /// complete.
    ///
    /// This is created by the `Future::join` method.
    (Join, <Fut1, Fut2>),

    /// Future for the `join3` combinator, waiting for three futures to
    /// complete.
    ///
    /// This is created by the `Future::join3` method.
    (Join3, <Fut1, Fut2, Fut3>),

    /// Future for the `join4` combinator, waiting for four futures to
    /// complete.
    ///
    /// This is created by the `Future::join4` method.
    (Join4, <Fut1, Fut2, Fut3, Fut4>),

    /// Future for the `join5` combinator, waiting for five futures to
    /// complete.
    ///
    /// This is created by the `Future::join5` method.
    (Join5, <Fut1, Fut2, Fut3, Fut4, Fut5>),
}
