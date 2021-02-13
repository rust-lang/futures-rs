//! Futures.

use core::ops::DerefMut;
use core::pin::Pin;

#[doc(no_inline)]
pub use core::future::Future;

/// A future which tracks whether or not the underlying future
/// should no longer be polled.
///
/// `is_terminated` will return `true` if a future should no longer be polled.
/// Usually, this state occurs after `poll` (or `try_poll`) returned
/// `Poll::Ready`. However, `is_terminated` may also return `true` if a future
/// has become inactive and can no longer make progress and should be ignored
/// or dropped rather than being `poll`ed again.
pub trait FusedFuture: Future {
    /// Returns `true` if the underlying future should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<F: FusedFuture + ?Sized + Unpin> FusedFuture for &mut F {
    fn is_terminated(&self) -> bool {
        <F as FusedFuture>::is_terminated(&**self)
    }
}

impl<P> FusedFuture for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: FusedFuture,
{
    fn is_terminated(&self) -> bool {
        <P::Target as FusedFuture>::is_terminated(&**self)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use alloc::boxed::Box;
    use super::*;

    impl<F: FusedFuture + ?Sized + Unpin> FusedFuture for Box<F> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }

    #[cfg(feature = "std")]
    impl<F: FusedFuture> FusedFuture for std::panic::AssertUnwindSafe<F> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }
}
