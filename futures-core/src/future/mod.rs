//! Futures.

use core::ops::DerefMut;
use core::pin::Pin;
use core::task::{Context, Poll};

pub use core::future::Future;

mod future_obj;
pub use self::future_obj::{FutureObj, LocalFutureObj, UnsafeFutureObj};

#[cfg(feature = "alloc")]
#[must_use = "futures do nothing unless polled"]
#[allow(missing_debug_implementations)]
/// An owned dynamically typed [`Future`] for use in cases where you can't
/// statically type your result or need to add some indirection.
pub struct BoxFuture<'a, T>(Pin<alloc::boxed::Box<dyn Future<Output = T> + Send + 'a>>);

#[cfg(feature = "alloc")]
#[must_use = "futures do nothing unless polled"]
#[allow(missing_debug_implementations)]
/// `BoxFuture`, but without the `Send` requirement.
pub struct LocalBoxFuture<'a, T>(Pin<alloc::boxed::Box<dyn Future<Output = T> + 'a>>);

/// A `Future` or `TryFuture` which tracks whether or not the underlying future
/// should no longer be polled.
///
/// `is_terminated` will return `true` if a future should no longer be polled.
/// Usually, this state occurs after `poll` (or `try_poll`) returned
/// `Poll::Ready`. However, `is_terminated` may also return `true` if a future
/// has become inactive and can no longer make progress and should be ignored
/// or dropped rather than being `poll`ed again.
pub trait FusedFuture {
    /// Returns `true` if the underlying future should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<F: FusedFuture + ?Sized> FusedFuture for &mut F {
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

/// A convenience for futures that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryFuture {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; in the future it won't be
    /// needed.
    fn try_poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Ok, Self::Error>>;
}

impl<F, T, E> TryFuture for F
    where F: ?Sized + Future<Output = Result<T, E>>
{
    type Ok = T;
    type Error = E;

    #[inline]
    fn try_poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        self.poll(cx)
    }
}

#[cfg(feature = "alloc")]
mod if_alloc {
    use core::mem;
    use alloc::boxed::Box;
    use super::*;

    impl<F: FusedFuture + ?Sized> FusedFuture for Box<F> {
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

    impl<O> Future for BoxFuture<'_, O> {
        type Output = O;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut_pin = self.0.as_mut();
            mut_pin.poll(cx)
        }
    }

    impl<O> BoxFuture<'_, O> {
        /// Converts Pin<Box<Future<Output=O>> to PinnedFuture<O>
        pub fn new<'a>(f: Pin<Box<dyn Future<Output = O> + Send + 'a>>) -> BoxFuture<'a, O> {
            BoxFuture(f)
        }
    }

    impl<O> Future for LocalBoxFuture<'_, O> {
        type Output = O;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut_pin = self.0.as_mut();
            mut_pin.poll(cx)
        }
    }

    impl<O> LocalBoxFuture<'_, O> {
        /// Converts Pin<Box<Future<Output=O>> to PinnedFuture<O>
        pub fn new<'a>(f: Pin<Box<dyn Future<Output = O> + 'a>>) -> LocalBoxFuture<'a, O> {
            LocalBoxFuture(f)
        }
    }

    unsafe impl<'a, T> UnsafeFutureObj<'a, T> for BoxFuture<'a, T> where T: 'a
    {
        fn into_raw(mut self) -> *mut (dyn Future<Output = T> + 'a) {
            let ptr = unsafe { self.0.as_mut().get_unchecked_mut() as *mut _ };
            mem::forget(self);
            ptr
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(Pin::from(Box::from_raw(ptr)))
        }
    }

    unsafe impl<'a, T> UnsafeFutureObj<'a, T> for LocalBoxFuture<'a, T> where T: 'a
    {
        fn into_raw(mut self) -> *mut (dyn Future<Output = T> + 'a) {
            let ptr = unsafe { self.0.as_mut().get_unchecked_mut() as *mut _ };
            mem::forget(self);
            ptr
        }

        unsafe fn drop(ptr: *mut (dyn Future<Output = T> + 'a)) {
            drop(Pin::from(Box::from_raw(ptr)))
        }
    }
}
