use core::mem::PinMut;

use futures_core::{Poll, Future, Stream};
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

impl<F: Future> IntoStream<F> {
    unsafe_pinned!(future -> Option<F>);
}

impl<F: Future> Stream for IntoStream<F> {
    type Item = F::Output;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        let v = match self.future().as_pin_mut() {
            Some(fut) => {
                match fut.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => v
                }
            }
            None => return Poll::Ready(None),
        };

        PinMut::set(self.future(), None);
        Poll::Ready(Some(v))
    }
}
