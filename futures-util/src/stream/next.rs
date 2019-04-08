use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

/// Future for the [`next`](super::StreamExt::next) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Next<'a, St> {
    stream: &'a mut St,
}

impl<St: Stream + Unpin> Unpin for Next<'_, St> {}

impl<'a, St: Stream + Unpin> Next<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        Next { stream }
    }
}

impl<St: FusedStream> FusedFuture for Next<'_, St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: Stream + Unpin> Future for Next<'_, St> {
    type Output = Option<St::Item>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        Pin::new(&mut *self.stream).poll_next(cx)
    }
}
