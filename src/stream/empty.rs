use core::marker;

use stream::Stream;
use {Poll, Async};

/// A stream which contains no elements.
///
/// This stream can be created with the `stream::empty` function.
pub struct Empty<T, E> {
    _data: marker::PhantomData<(T, E)>,
}

/// Creates a stream which contains no elements.
///
/// The returned stream will always return `Ready(None)` when polled.
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
        Ok(Async::Ready(None))
    }
}
