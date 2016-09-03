use core::marker;

use {Future, Poll};

/// A future which is never resolved.
///
/// This future can be created with the `never` function.
#[derive(Copy, Clone)]
pub struct Never<T=!, E=!> {
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will never resolve with a success but is still
/// susceptible to cancellation. That is, if a callback is scheduled on the
/// returned future, it is only run once the future is dropped (canceled).
pub fn never<T=!, E=!>() -> Never<T, E> {
    Never { _data: marker::PhantomData }
}

impl<T, E> Future for Never<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<!, !> {
        Poll::NotReady
    }
}

/// A future representing a finished but erroneous computation.
///
/// Created by the `failed` function.
pub struct Failed<E, T=!> {
    _t: marker::PhantomData<T>,
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
pub fn failed<E, T=!>(e: E) -> Failed<T, E> {
    Failed { t: marker::PhantomData, e: Some(e) }
}

impl<T, E> Future for Failed<T,E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<!, E> {
        Poll::Err(self.e.take().expect("cannot poll Failed twice"))
    }
}

/// A future representing a finished successful computation.
///
/// Created by the `finished` function.
pub struct Finished<T, E=!> {
    t: Option<T>,
    _e: marker::PhantomData<E>,
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
pub fn finished<T, E=!>(t: T) -> Finished<T, E> {
    Finished { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for Finished<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        Poll::Ok(self.t.take().expect("cannot poll Finished twice"))
    }
}
