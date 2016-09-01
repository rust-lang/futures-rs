use {Future, Poll, Async};

/// A future representing a value that is immediately ready.
///
/// Created by the `done` function.
pub struct Done<T, E> {
    inner: Option<Result<T, E>>,
}

/// Creates a new "leaf future" which will resolve with the given result.
///
/// The returned future represents a computation which is finshed immediately.
/// This can be useful with the `finished` and `failed` base future types to
/// convert an immediate value to a future to interoperate elsewhere.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let future_of_1 = done::<u32, u32>(Ok(1));
/// let future_of_err_2 = done::<u32, u32>(Err(2));
/// ```
pub fn done<T, E>(r: Result<T, E>) -> Done<T, E> {
    Done { inner: Some(r) }
}

impl<T, E> Future for Done<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        self.inner.take().expect("cannot poll Done twice").map(Async::Ready)
    }
}
