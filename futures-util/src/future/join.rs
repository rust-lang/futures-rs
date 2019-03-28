#![allow(non_snake_case)]

use crate::future::{MaybeDone, maybe_done};
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
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
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
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

            #[allow(clippy::useless_let_if_seq)]
            fn poll(
                mut self: Pin<&mut Self>, waker: &Waker
            ) -> Poll<Self::Output> {
                let mut all_done = true;
                $(
                    if self.as_mut().$Fut().poll(waker).is_pending() {
                        all_done = false;
                    }
                )*

                if all_done {
                    Poll::Ready(($(self.as_mut().$Fut().take_output().unwrap()), *))
                } else {
                    Poll::Pending
                }
            }
        }
    )*)
}

generate! {
    /// Future for the [`join`](super::FutureExt::join) combinator.
    (Join, <Fut1, Fut2>),

    /// Future for the [`join3`](super::FutureExt::join3) combinator.
    (Join3, <Fut1, Fut2, Fut3>),

    /// Future for the [`join4`](super::FutureExt::join4) combinator.
    (Join4, <Fut1, Fut2, Fut3, Fut4>),

    /// Future for the [`join5`](super::FutureExt::join5) combinator.
    (Join5, <Fut1, Fut2, Fut3, Fut4, Fut5>),
}
