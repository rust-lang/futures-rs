use futures_core::stream::Stream;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// Stream for the [`interleave_pending`](super::StreamTestExt::interleave_pending) method.
#[derive(Debug)]
pub struct InterleavePending<St: Stream> {
    stream: St,
    pended: bool,
}

impl<St: Stream + Unpin> Unpin for InterleavePending<St> {}

impl<St: Stream> InterleavePending<St> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(pended: bool);

    pub(crate) fn new(stream: St) -> InterleavePending<St> {
        InterleavePending {
            stream,
            pended: false,
        }
    }
}

impl<St: Stream> Stream for InterleavePending<St> {
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if *self.as_mut().pended() {
            let next = self.as_mut().stream().poll_next(cx);
            if next.is_ready() {
                *self.pended() = false;
            }
            next
        } else {
            cx.waker().wake_by_ref();
            *self.pended() = true;
            Poll::Pending
        }
    }
}
