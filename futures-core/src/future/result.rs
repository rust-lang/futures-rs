use {Future, IntoFuture, Poll, Async};
use task;
use core::result;

/// A future representing a value that is immediately ready.
///
/// Created by the `result` function.
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

/// Creates a new "leaf future" which will resolve with the given result.
///
/// The returned future represents a computation which is finished immediately.
/// This can be useful with the `finished` and `failed` base future types to
/// convert an immediate value to a future to interoperate elsewhere.
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

/// Creates a "leaf future" from an immediate value of a finished and
/// successful computation.
///
/// The returned future is similar to `result` where it will immediately run a
/// scheduled callback with the provided value.
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

/// Creates a "leaf future" from an immediate value of a failed computation.
///
/// The returned future is similar to `result` where it will immediately run a
/// scheduled callback with the provided value.
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
