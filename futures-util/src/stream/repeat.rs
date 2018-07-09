use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

/// Stream that produces the same element repeatedly.
///
/// This structure is created by the `stream::repeat` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Repeat<T> {
    item: T,
}

/// Create a stream which produces the same item repeatedly.
///
/// The stream never terminates. Note that you likely want to avoid
/// usage of `collect` or such on the returned stream as it will exhaust
/// available memory as it tries to just fill up all RAM.
///
/// ```rust
/// # extern crate futures;
/// use futures::prelude::*;
/// use futures::stream;
/// use futures::executor::block_on;
///
/// let mut stream = stream::repeat(9);
/// assert_eq!(vec![9, 9, 9], block_on(stream.take(3).collect::<Vec<i32>>()));
/// ```
pub fn repeat<T>(item: T) -> Repeat<T>
    where T: Clone
{
    Repeat { item }
}

impl<T> Unpin for Repeat<T> {}

impl<T> Stream for Repeat<T>
    where T: Clone
{
    type Item = T;

    fn poll_next(self: PinMut<Self>, _: &mut task::Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(self.item.clone()))
    }
}
