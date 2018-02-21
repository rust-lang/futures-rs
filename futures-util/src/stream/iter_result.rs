use futures_core::{Async, Poll, Stream};
use futures_core::task;

/// A stream which is just a shim over an underlying instance of `Iterator`.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct IterResult<I> {
    iter: I,
}

/// Converts an `Iterator` over `Result`s into a `Stream` which is always ready
/// to yield the next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter simply
/// always calls `iter.next()` and returns that.
///
/// ```rust
/// # extern crate futures;
/// # extern crate futures_executor;
/// use futures::prelude::*;
/// use futures::stream;
/// use futures_executor::block_on;
///
/// # fn main() {
/// let mut stream = stream::iter_result(vec![Ok(17), Err(false), Ok(19)]);
/// let (item, stream) = block_on(stream.into_future()).unwrap();
/// assert_eq!(Some(17), item);
/// let (err, stream) = block_on(stream.into_future()).unwrap_err();
/// assert_eq!(false, err);
/// let (item, stream) = block_on(stream.into_future()).unwrap();
/// assert_eq!(Some(19), item);
/// let (item, _) = block_on(stream.into_future()).unwrap();
/// assert_eq!(None, item);
/// # }
/// ```
pub fn iter_result<J, T, E>(i: J) -> IterResult<J::IntoIter>
where
    J: IntoIterator<Item = Result<T, E>>,
{
    IterResult {
        iter: i.into_iter(),
    }
}

impl<I, T, E> Stream for IterResult<I>
where
    I: Iterator<Item = Result<T, E>>,
{
    type Item = T;
    type Error = E;

    fn poll_next(&mut self, _: &mut task::Context) -> Poll<Option<T>, E> {
        match self.iter.next() {
            Some(Ok(e)) => Ok(Async::Ready(Some(e))),
            Some(Err(e)) => Err(e),
            None => Ok(Async::Ready(None)),
        }
    }
}
