use std::marker;

use {Future, Task, Poll};

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
pub struct Empty<T, E>
    where T: 'static,
          E: 'static,
{
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will never resolve with a success but is still
/// susceptible to cancellation. That is, if a callback is scheduled on the
/// returned future, it is only run once the future is dropped (canceled).
pub fn empty<T: 'static, E: 'static>() -> Empty<T, E> {
    Empty { _data: marker::PhantomData }
}

impl<T, E> Future for Empty<T, E>
    where T: 'static,
          E: 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self, _: &mut Task) -> Poll<T, E> {
        Poll::NotReady
    }

    fn schedule(&mut self, _task: &mut Task) {}
}
