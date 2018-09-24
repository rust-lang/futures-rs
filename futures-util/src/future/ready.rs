use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

/// A future that is immediately ready with a value
///
/// Created by the [`ready()`] function.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Ready<T>(Option<T>);

impl<T> Unpin for Ready<T> {}

impl<T> Future for Ready<T> {
    type Output = T;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll<T> {
        Poll::Ready(self.0.take().unwrap())
    }
}

/// Create a future that is immediately ready with a value.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::ready(1);
/// assert_eq!(await!(a), 1);
/// # });
/// ```
pub fn ready<T>(t: T) -> Ready<T> {
    Ready(Some(t))
}

/// Create a future that is immediately ready with a success value.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::ok::<i32, i32>(1);
/// assert_eq!(await!(a), Ok(1));
/// # });
/// ```
pub fn ok<T, E>(t: T) -> Ready<Result<T, E>> {
    Ready(Some(Ok(t)))
}

/// Create a future that is immediately ready with an error value.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let a = future::err::<i32, i32>(1);
/// assert_eq!(await!(a), Err(1));
/// # });
/// ```
pub fn err<T, E>(err: E) -> Ready<Result<T, E>> {
    Ready(Some(Err(err)))
}
