use core::marker;

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
/// The returned future will forever return `Poll::NotReady`.
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
