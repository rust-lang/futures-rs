use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};

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
/// use futures::prelude::*;
/// use futures::future;
/// use futures::executor::block_on;
/// use futures::stream;
///
/// let mut stream = stream::once(future::ready(17));
/// let collected = block_on(stream.collect::<Vec<i32>>());
/// assert_eq!(collected, vec![17]);
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
