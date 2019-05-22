use crate::try_stream::TryStreamExt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{Context, Poll};

/// Future for the [`try_next`](super::TryStreamExt::try_next) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
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
        self.stream.try_poll_next_unpin(cx)?.map(Ok)
    }
}
