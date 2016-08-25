use {Future, Poll};

/// A future which is never resolved.
///
/// This future can be created with the `empty` function.
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
