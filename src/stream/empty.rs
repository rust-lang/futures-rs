use core::marker;

use stream::Stream;
use Poll;

/// A stream which is never resolved.
///
/// This stream can be created with the `stream::empty` function.
pub struct Empty<T, E> {
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a stream which never resolves, representing a computation that never
/// finishes.
///
/// The returned stream will never resolve with a value but is still
/// susceptible to cancellation. That is, if a callback is scheduled on the
/// returned stream, it is only run once the stream is dropped (canceled).
pub fn empty<T, E>() -> Empty<T, E> {
    Empty { _data: marker::PhantomData }
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
