use {Async, Poll};
use stream::Stream;

/// A stream which is just a shim over an underlying instance of `Iterator`.
///
/// This stream will never block and is always ready.
pub struct IterStream<I> {
    iter: I,
}

/// Converts an `Iterator` into a `Stream` which is always ready to yield the
/// next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter simply
/// always calls `iter.next()` and returns that. Additionally, the error type is
/// generic here as it will never be returned, instead the type of the iterator
/// will always be returned upwards as a successful value.
pub fn iter<I, T, E>(i: I) -> IterStream<I>
    where I: Iterator<Item=Result<T, E>>,
{
    IterStream {
        iter: i,
    }
}

impl<I, T, E> Stream for IterStream<I>
    where I: Iterator<Item=Result<T, E>>,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<T>, E> {
        match self.iter.next() {
            Some(Ok(e)) => Ok(Async::Ready(Some(e))),
            Some(Err(e)) => Err(e),
            None => Ok(Async::Ready(None)),
        }
    }
}
