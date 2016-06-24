use std::marker;
use std::sync::Arc;

use {Future, Wake, PollResult};

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
pub struct Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will never resolve with a success but is still
/// susceptible to cancellation. That is, if a callback is scheduled on the
/// returned future, it is only run once the future is dropped (canceled).
pub fn empty<T: Send + 'static, E: Send + 'static>() -> Empty<T, E> {
    Empty { _data: marker::PhantomData }
}

impl<T, E> Future for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<PollResult<T, E>> {
        None
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        drop(wake)
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=T, Error=E>>> {
        None
    }
}
