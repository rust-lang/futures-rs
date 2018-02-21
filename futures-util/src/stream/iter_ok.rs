use core::marker;

use futures_core::{Async, Poll, Stream};
use futures_core::task;

/// A stream which is just a shim over an underlying instance of `Iterator`.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct IterOk<I, E> {
    iter: I,
    _marker: marker::PhantomData<fn() -> E>,
}

/// Converts an `Iterator` into a `Stream` which is always ready
/// to yield the next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter
/// simply always calls `iter.next()` and returns that.
///
/// ```rust
/// # extern crate futures;
/// # extern crate futures_executor;
/// use futures::prelude::*;
/// use futures::stream;
/// use futures_executor::block_on;
///
/// # fn main() {
/// let mut stream = stream::iter_ok::<_, ()>(vec![17, 19]);
/// assert_eq!(Ok(vec![17, 19]), block_on(stream.collect()));
/// # }
/// ```
pub fn iter_ok<I, E>(i: I) -> IterOk<I::IntoIter, E>
    where I: IntoIterator,
{
    IterOk {
        iter: i.into_iter(),
        _marker: marker::PhantomData,
    }
}

impl<I, E> Stream for IterOk<I, E>
    where I: Iterator,
{
    type Item = I::Item;
    type Error = E;

    fn poll_next(&mut self, _: &mut task::Context) -> Poll<Option<I::Item>, E> {
        Ok(Async::Ready(self.iter.next()))
    }
}
