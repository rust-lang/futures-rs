use std::marker;

use {Future, Poll};

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
pub struct Empty<T, E> {
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will never resolve with a success but is still
/// susceptible to cancellation. That is, if a callback is scheduled on the
/// returned future, it is only run once the future is dropped (canceled).
pub fn empty<T, E>() -> Empty<T, E> {
    Empty { _data: marker::PhantomData }
}

impl<T, E> Future for Empty<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<T, E> {
        Poll::NotReady
    }
}
