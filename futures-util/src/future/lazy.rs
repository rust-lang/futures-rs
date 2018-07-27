use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// A future which, when polled, invokes a closure and yields its result.
///
/// This is created by the [`lazy()`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Lazy<F> {
    f: Option<F>
}

// safe because we never generate `PinMut<F>`
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
    where F: FnOnce(&mut task::Context) -> R,
{
    Lazy { f: Some(f) }
}

impl<R, F> Future for Lazy<F>
    where F: FnOnce(&mut task::Context) -> R,
{
    type Output = R;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<R> {
        Poll::Ready((self.f.take().unwrap())(cx))
    }
}
