use futures_core::{Poll, Async, Stream, IntoFuture, Future};
use futures_core::task;

/// A stream which emits single element and then EOF.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Once<F>(Option<F>);

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
pub fn once<F: IntoFuture>(item: F) -> Once<F::Future> {
    Once(Some(item.into_future()))
}

impl<F: Future> Stream for Once<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<F::Item>, F::Error> {
        if let Some(mut f) = self.0.take() {
            match f.poll(cx)? {
                Async::Ready(x) => Ok(Async::Ready(Some(x))),
                Async::Pending => {
                    self.0 = Some(f);
                    Ok(Async::Pending)
                }
            }
        } else {
            Ok(Async::Ready(None))
        }
    }
}
