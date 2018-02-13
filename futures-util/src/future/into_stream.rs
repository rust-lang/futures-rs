use futures_core::{Async, Poll, Future, Stream};
use futures_core::task;

/// Future that forwards one element from the underlying future
/// (whether it is success of error) and emits EOF after that.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct IntoStream<F: Future> {
    future: Option<F>
}

pub fn new<F: Future>(future: F) -> IntoStream<F> {
    IntoStream {
        future: Some(future)
    }
}

impl<F: Future> Stream for IntoStream<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        let ret = match self.future {
            None => return Ok(Async::Ready(None)),
            Some(ref mut future) => {
                match future.poll(cx) {
                    Ok(Async::Pending) => return Ok(Async::Pending),
                    Err(e) => Err(e),
                    Ok(Async::Ready(r)) => Ok(r),
                }
            }
        };
        self.future = None;
        ret.map(|r| Async::Ready(Some(r)))
    }
}
