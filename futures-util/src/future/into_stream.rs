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

impl<F: Future> Stream for IntoStream<F> {
    type Item = F::Output;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        // safety: we use this &mut only for matching, not for movement
        let v = match &mut unsafe { PinMut::get_mut(self.reborrow()) }.future {
            Some(fut) => {
                // safety: this re-pinned future will never move before being dropped
                match unsafe { PinMut::new_unchecked(fut) }.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => v
                }
            }
            None => return Poll::Ready(None),
        };

        // safety: we use this &mut only for a replacement, which drops the future in place
        unsafe { PinMut::get_mut(self) }.future = None;
        Poll::Ready(Some(v))
    }
}
