use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};
use core::pin::Pin;

/// Future for the [`try_next`](super::TryStreamExt::try_next) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryNext<'a, St: Unpin> {
    stream: &'a mut St,
}

impl<St: Unpin> Unpin for TryNext<'_, St> {}

impl<'a, St: TryStream + Unpin> TryNext<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        TryNext { stream }
    }
}

impl<St: Unpin + FusedStream> FusedFuture for TryNext<'_, St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: TryStream + Unpin> Future for TryNext<'_, St> {
    type Output = Result<Option<St::Ok>, St::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        match Pin::new(&mut *self.stream).try_poll_next(cx) {
            Poll::Ready(Some(Ok(x))) => Poll::Ready(Ok(Some(x))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Err(e)),
            Poll::Ready(None) => Poll::Ready(Ok(None)),
            Poll::Pending => Poll::Pending,
        }
    }
}
