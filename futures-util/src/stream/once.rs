use futures_core::{Poll, Async, Stream};
use futures_core::task;

/// A stream which emits single element and then EOF.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Once<T, E>(Option<Result<T, E>>);

/// Creates a stream of single element
///
/// ```rust
/// # extern crate futures;
/// # extern crate futures_executor;
/// use futures::prelude::*;
/// use futures::stream;
/// use futures_executor::block_on;
///
/// # fn main() {
/// let mut stream = stream::once::<(), _>(Err(17));
/// let collected: Result<Vec<_>, _> = block_on(stream.collect());
/// assert_eq!(collected, Err(17));
///
/// let mut stream = stream::once::<_, ()>(Ok(92));
/// let collected: Result<Vec<_>, _> = block_on(stream.collect());
/// assert_eq!(collected, Ok(vec![92]));
/// # }
/// ```
pub fn once<T, E>(item: Result<T, E>) -> Once<T, E> {
    Once(Some(item))
}

impl<T, E> Stream for Once<T, E> {
    type Item = T;
    type Error = E;

    fn poll_next(&mut self, _: &mut task::Context) -> Poll<Option<T>, E> {
        match self.0.take() {
            Some(Ok(e)) => Ok(Async::Ready(Some(e))),
            Some(Err(e)) => Err(e),
            None => Ok(Async::Ready(None)),
        }
    }
}
