use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncRead, AsyncWrite};
use std::io;
use std::pin::Pin;
use super::{BufReader, CopyBufInto};
use pin_utils::unsafe_pinned;

/// Future for the [`copy_into`](super::AsyncReadExt::copy_into) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct CopyInto<R: AsyncRead, W> {
    inner: CopyBufInto<BufReader<R>, W>,
}

impl<R: AsyncRead, W> Unpin for CopyInto<R, W> where CopyBufInto<BufReader<R>, W>: Unpin {}

impl<R: AsyncRead, W> CopyInto<R, W> {
    unsafe_pinned!(inner: CopyBufInto<BufReader<R>, W>);

    pub(super) fn new(reader: R, writer: W) -> Self {
        CopyInto {
            inner: CopyBufInto::new(BufReader::new(reader), writer),
        }
    }
}

impl<R: AsyncRead, W: AsyncWrite> Future for CopyInto<R, W> {
    type Output = io::Result<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner().poll(cx)
    }
}
