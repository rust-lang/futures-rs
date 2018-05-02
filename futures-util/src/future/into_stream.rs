use futures_core::{Poll, Future, Stream};
use futures_core::task;

#[cfg(feature = "nightly")]
use core::mem::Pin;

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

#[cfg(feature = "nightly")]
impl<F: Future> Stream for IntoStream<F> {
    type Item = F::Output;

    fn poll_next(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        // safety: we use this &mut only for matching, not for movement
        let v = match unsafe { Pin::get_mut(&mut self) }.future {
            Some(ref mut fut) => {
                // safety: this re-pinned future will never move before being dropped
                match unsafe { Pin::new_unchecked(fut) }.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(v) => v
                }
            }
            None => return Poll::Ready(None),
        };

        // safety: we use this &mut only for a replacement, which drops the future in place
        unsafe { Pin::get_mut(&mut self) }.future = None;
        Poll::Ready(Some(v))
    }
}

#[cfg(not(feature = "nightly"))]
unpinned! {
    impl<F: Future + ::futures_core::Unpin> Stream for IntoStream<F> {
        type Item = F::Output;

        fn poll_next_mut(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
            let v = match self.future {
                Some(ref mut fut) => {
                    match fut.poll_mut(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(v) => v
                    }
                }
                None => return Poll::Ready(None),
            };
            self.future = None;
            Poll::Ready(Some(v))
        }
    }
}
