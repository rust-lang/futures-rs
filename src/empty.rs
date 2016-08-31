use core::marker;

use {Future, Poll};
use stream::Stream;

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

impl<T, E> Stream for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        Poll::Ok(None)
    }
}
