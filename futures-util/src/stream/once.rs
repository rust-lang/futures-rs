use core::mem::PinMut;



use futures_core::{Poll, Stream, Future};
use futures_core::task;

/// A stream which emits single element and then EOF.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Once<F> {
    fut: Option<F>
}

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
/// let mut stream = stream::once::<Result<(), _>>(Err(17));
/// let collected: Result<Vec<_>, _> = block_on(stream.collect());
/// assert_eq!(collected, Err(17));
///
/// let mut stream = stream::once::<Result<_, ()>>(Ok(92));
/// let collected: Result<Vec<_>, _> = block_on(stream.collect());
/// assert_eq!(collected, Ok(vec![92]));
/// # }
/// ```
pub fn once<F: Future>(item: F) -> Once<F> {
    Once { fut: Some(item) }
}

impl<F> Once<F> {
    unsafe_pinned!(fut -> Option<F>);
}

impl<F: Future> Stream for Once<F> {
    type Item = F::Output;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<F::Output>> {
        let val = if let Some(f) = self.fut().as_pin_mut() {
            ready!(f.poll(cx))
        } else {
            return Poll::Ready(None)
        };
        PinMut::set(self.fut(), None);
        Poll::Ready(Some(val))
    }
}
