use core::mem::PinMut;
use core::marker::Unpin;

use futures_core::{Stream, Poll};
use futures_core::task;

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
/// # extern crate futures_executor;
/// use futures::prelude::*;
/// use futures::stream;
/// use futures_executor::block_on;
///
/// # fn main() {
/// let mut stream = stream::repeat::<_, bool>(10);
/// assert_eq!(Ok(vec![10, 10, 10]), block_on(stream.take(3).collect()));
/// # }
/// ```
pub fn repeat<T>(item: T) -> Repeat<T>
    where T: Clone
{
    Repeat {
        item: item,
    }
}

unsafe impl<T> Unpin for Repeat<T> {}

impl<T> Stream for Repeat<T>
    where T: Clone
{
    type Item = T;

    fn poll_next(self: PinMut<Self>, _: &mut task::Context) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(self.item.clone()))
    }
}
