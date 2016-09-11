use core::marker;

use {Future, Poll, Async};

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

    fn poll(&mut self) -> Poll<T, E> {
        Ok(Async::NotReady)
    }
}

/// A future representing a finished but erroneous computation.
///
/// Created by the `always_failed` function.
pub struct AlwaysFailed<E, T=!> {
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
///#![feature(default_type_parameter_fallback)]
/// use futures::*;
///
/// let future_of_err_1 = always_failed(1);
/// ```
pub fn always_failed<E, T=!>(e: E) -> AlwaysFailed<E, T> {
    AlwaysFailed { _t: marker::PhantomData, e: Some(e) }
}

impl<E, T> Future for AlwaysFailed<E, T> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        Err(self.e.take().expect("cannot poll AlwaysFailed twice"))
    }
}

/// A future representing a finished successful computation.
///
/// Created by the `always_finished` function.
pub struct AlwaysFinished<T, E=!> {
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
///#![feature(default_type_parameter_fallback)]
/// use futures::*;
///
/// let future_of_1 = always_finished(1);
/// ```
pub fn always_finished<T, E=!>(t: T) -> AlwaysFinished<T, E> {
    AlwaysFinished { t: Some(t), _e: marker::PhantomData }
}

impl<T, E> Future for AlwaysFinished<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        Ok(Async::Ready(self.t.take().expect("cannot poll AlwaysFinished twice")))
    }
}
