use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Waker, Poll};

/// Future for the [`lazy`] combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Lazy<F> {
    f: Option<F>
}

// safe because we never generate `Pin<&mut F>`
impl<F> Unpin for Lazy<F> {}

/// Creates a new future that allows delayed execution of a closure.
///
/// The provided closure is only run once the future is polled.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::lazy(|_| 1);
/// assert_eq!(await!(a), 1);
///
/// let b = future::lazy(|_| -> i32 {
///     panic!("oh no!")
/// });
/// drop(b); // closure is never run
/// # });
/// ```
pub fn lazy<F, R>(f: F) -> Lazy<F>
    where F: FnOnce(&Waker) -> R,
{
    Lazy { f: Some(f) }
}

impl<F> FusedFuture for Lazy<F> {
    fn is_terminated(&self) -> bool { self.f.is_none() }
}

impl<R, F> Future for Lazy<F>
    where F: FnOnce(&Waker) -> R,
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<R> {
        Poll::Ready((self.f.take().unwrap())(waker))
    }
}
