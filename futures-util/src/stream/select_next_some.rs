use core::pin::Pin;
use futures_core::stream::{Stream, FusedStream};
use futures_core::future::{Future, FusedFuture};
use futures_core::task::{Context, Poll};
use crate::stream::StreamExt;

/// Future for the [`select_next_some`](super::StreamExt::select_next_some)
/// method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct SelectNextSome<'a, St> {
    stream: &'a mut St,
}

impl<'a, St> SelectNextSome<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        SelectNextSome { stream }
    }
}

impl<St: FusedStream> FusedFuture for SelectNextSome<'_, St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: Stream + FusedStream + Unpin> Future for SelectNextSome<'_, St> {
    type Output = St::Item;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        assert!(!self.stream.is_terminated(), "SelectNextSome polled after terminated");

        if let Some(item) = ready!(self.stream.poll_next_unpin(cx)) {
            Poll::Ready(item)
        } else {
            debug_assert!(self.stream.is_terminated());
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
