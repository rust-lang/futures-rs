use {Future, IntoFuture, Poll, Async};
use task;
use core::result;

/// A future representing a value that is immediately ready.
///
/// Created by the [`result`](::future::result), [`ok`](::future::ok) or
/// [`err`](::future::err) functions.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Result<T, E> {
    inner: Option<result::Result<T, E>>,
}

impl<T, E> IntoFuture for result::Result<T, E> {
    type Future = Result<T, E>;
    type Item = T;
    type Error = E;

    fn into_future(self) -> Result<T, E> {
        result(self)
    }
}

/// Creates a new future that will immediate resolve with the given result.
///
/// # Examples
///
/// ```
/// use futures_core::future::*;
///
/// let future_of_1 = result::<u32, u32>(Ok(1));
/// let future_of_err_2 = result::<u32, u32>(Err(2));
/// ```
pub fn result<T, E>(r: result::Result<T, E>) -> Result<T, E> {
    Result { inner: Some(r) }
}

/// Creates a new future that will immediately resolve successfully to the given value.
///
/// # Examples
///
/// ```
/// use futures_core::future::*;
///
/// let future_of_1 = ok::<u32, u32>(1);
/// ```
pub fn ok<T, E>(t: T) -> Result<T, E> {
    result(Ok(t))
}

/// Creates a new future that will immediately fail with the given error.
///
/// # Examples
///
/// ```
/// use futures_core::future::*;
///
/// let future_of_err_1 = err::<u32, u32>(1);
/// ```
pub fn err<T, E>(e: E) -> Result<T, E> {
    result(Err(e))
}

impl<T, E> Future for Result<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self, _: &mut task::Context) -> Poll<T, E> {
        self.inner.take().expect("cannot poll Result twice").map(Async::Ready)
    }
}

impl<T, E> From<result::Result<T, E>> for Result<T, E> {
    fn from(r: result::Result<T, E>) -> Self {
        result(r)
    }
}
