use futures_core::{Poll, PollResult, Future, Stream};
use futures_core::task;

/// A type which converts a `Future` into a `Stream`
/// containing a single element.
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

    fn poll_next(&mut self, cx: &mut task::Context) -> PollResult<Option<Self::Item>, Self::Error> {
        let ret = match self.future {
            None => return Poll::Ready(Ok(None)),
            Some(ref mut future) => {
                match future.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(r) => r,
                }
            }
        };
        self.future = None;
        Poll::Ready(ret.map(Some))
    }
}
