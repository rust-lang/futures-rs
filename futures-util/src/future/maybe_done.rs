//! Definition of the MaybeDone combinator

use core::marker::Unpin;
use core::mem::{self, PinMut};
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// A future that may have completed.
///
/// This is created by the [`maybe_done()`] function.
#[derive(Debug)]
pub enum MaybeDone<Fut: Future> {
    /// A not-yet-completed future
    Future(Fut),
    /// The output of the completed future
    Done(Fut::Output),
    /// The empty variant after the result of a [`MaybeDone`] has been
    /// taken using the [`take_output`](MaybeDone::take_output) method.
    Gone,
}

// Safe because we never generate `PinMut<Fut::Output>`
impl<Fut: Future + Unpin> Unpin for MaybeDone<Fut> {}

/// Wraps a future into a `MaybeDone`
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api, use_extern_macros, pin)]
/// # futures::executor::block_on(async {
/// use futures::future;
/// use pin_utils::pin_mut;
///
/// let future = future::maybe_done(future::ready(5));
/// pin_mut!(future);
/// assert_eq!(future.reborrow().take_output(), None);
/// let () = await!(future.reborrow());
/// assert_eq!(future.reborrow().take_output(), Some(5));
/// assert_eq!(future.reborrow().take_output(), None);
/// # });
/// ```
pub fn maybe_done<Fut: Future>(future: Fut) -> MaybeDone<Fut> {
    MaybeDone::Future(future)
}

impl<Fut: Future> MaybeDone<Fut> {
    /// Returns an [`Option`] containing a mutable reference to the output of the future.
    /// The output of this method will be [`Some`] if and only if the inner
    /// future has been completed and [`take_output`](MaybeDone::take_output)
    /// has not yet been called.
    #[inline]
    #[allow(needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    pub fn output_mut<'a>(self: PinMut<'a, Self>) -> Option<&'a mut Fut::Output> {
        unsafe {
            let this = PinMut::get_mut_unchecked(self);
            match this {
                MaybeDone::Done(res) => Some(res),
                _ => None,
            }
        }
    }

    /// Attempt to take the output of a `MaybeDone` without driving it
    /// towards completion.
    #[inline]
    pub fn take_output(self: PinMut<Self>) -> Option<Fut::Output> {
        unsafe {
            let this = PinMut::get_mut_unchecked(self);
            match this {
                MaybeDone::Done(_) => {},
                MaybeDone::Future(_) | MaybeDone::Gone => return None,
            };
            if let MaybeDone::Done(output) = mem::replace(this, MaybeDone::Gone) {
                Some(output)
            } else {
                unreachable!()
            }
        }
    }
}

impl<Fut: Future> Future for MaybeDone<Fut> {
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let res = unsafe {
            match PinMut::get_mut_unchecked(self.reborrow()) {
                MaybeDone::Future(a) => {
                    if let Poll::Ready(res) = PinMut::new_unchecked(a).poll(cx) {
                        res
                    } else {
                        return Poll::Pending
                    }
                }
                MaybeDone::Done(_) => return Poll::Ready(()),
                MaybeDone::Gone => panic!("MaybeDone polled after value taken"),
            }
        };
        PinMut::set(self, MaybeDone::Done(res));
        Poll::Ready(())
    }
}
