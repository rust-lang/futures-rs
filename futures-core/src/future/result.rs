use {Future, IntoFuture, Poll, Async};
use task;

/// A future representing a value that is immediately ready.
///
/// Created by the [`result`](::future::result), [`ok`](::future::ok) or
/// [`err`](::future::err) functions.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct FutureResult<T, E> {
    inner: Option<Result<T, E>>,
}

impl<T, E> IntoFuture for Result<T, E> {
    type Future = FutureResult<T, E>;
    type Item = T;
    type Error = E;

    fn into_future(self) -> Self::Future {
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
pub fn result<T, E>(r: Result<T, E>) -> FutureResult<T, E> {
    FutureResult { inner: Some(r) }
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
pub fn ok<T, E>(t: T) -> FutureResult<T, E> {
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
pub fn err<T, E>(e: E) -> FutureResult<T, E> {
    result(Err(e))
}

impl<T, E> Future for FutureResult<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self, _: &mut task::Context) -> Poll<T, E> {
        self.inner.take().expect("cannot poll Result twice").map(Async::Ready)
    }
}

impl<T, E> From<Result<T, E>> for FutureResult<T, E> {
    fn from(r: Result<T, E>) -> Self {
        result(r)
    }
}
