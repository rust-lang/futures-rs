use {Future, Poll};

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
#[derive(Copy, Clone)]
pub struct Never {}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will never resolve with a success but is still
/// susceptible to cancellation. That is, if a callback is scheduled on the
/// returned future, it is only run once the future is dropped (canceled).
pub fn never<T, E>() -> impl Future<Item = T, Error = E> + Send + 'static {
    (Never {}).map(|x| x).map_err(|x| x)
}

impl Future for Never {
    type Item = !;
    type Error = !;

    fn poll(&mut self) -> Poll<!, !> {
        Poll::NotReady
    }
}

/// A future representing a finished but erroneous computation.
///
/// Created by the `failed` function.
pub struct Failed<E> {
    e: Option<E>,
}

/// Creates a "leaf future" from an immediate value of a failed computation.
///
/// The returned future is similar to `done` where it will immediately run a
/// scheduled callback with the provided value.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let future_of_err_1 = failed::<u32, u32>(1);
/// ```
pub fn failed<T, E>(e: E) -> impl Future<Item = T, Error = E> {
    Failed { e: Some(e) }.map(|x| x)
}

impl<E> Future for Failed<E> {
    type Item = !;
    type Error = E;

    fn poll(&mut self) -> Poll<!, E> {
        Poll::Err(self.e.take().expect("cannot poll Failed twice"))
    }
}

/// A future representing a finished successful computation.
///
/// Created by the `finished` function.
pub struct Finished<T> {
    t: Option<T>,
}

/// Creates a "leaf future" from an immediate value of a finished and
/// successful computation.
///
/// The returned future is similar to `done` where it will immediately run a
/// scheduled callback with the provided value.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let future_of_1 = finished::<u32, u32>(1);
/// ```
pub fn finished<T, E>(t: T) -> impl Future<Item = T, Error = E> {
    Finished { t: Some(t) }.map_err(|x| x)
}

impl<T> Future for Finished<T> {
    type Item = T;
    type Error = !;

    fn poll(&mut self) -> Poll<T, !> {
        Poll::Ok(self.t.take().expect("cannot poll Finished twice"))
    }
}
