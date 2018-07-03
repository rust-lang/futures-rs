use core::mem::PinMut;
use core::marker::Unpin;

use futures_core::{Poll, Stream};
use futures_core::task;

/// A stream which is just a shim over an underlying instance of `Iterator`.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Iter<I> {
    iter: I,
}

impl<I> Unpin for Iter<I> {}

/// Converts an `Iterator` into a `Stream` which is always ready
/// to yield the next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter
/// simply always calls `iter.next()` and returns that.
///
/// ```rust
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::stream;
/// use futures::executor::block_on;
///
/// let mut stream = stream::iter(vec![17, 19]);
/// assert_eq!(vec![17, 19], block_on(stream.collect::<Vec<i32>>()));
/// ```
pub fn iter<I>(i: I) -> Iter<I::IntoIter>
    where I: IntoIterator,
{
    Iter {
        iter: i.into_iter(),
    }
}

impl<I> Stream for Iter<I>
    where I: Iterator,
{
    type Item = I::Item;

    fn poll_next(mut self: PinMut<Self>, _: &mut task::Context) -> Poll<Option<I::Item>> {
        Poll::Ready(self.iter.next())
    }
}
