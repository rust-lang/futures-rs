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
pub struct CopyInto<'a, R: AsyncRead, W: ?Sized> {
    inner: CopyBufInto<'a, BufReader<R>, W>,
}

impl<'a, R: AsyncRead, W: ?Sized> Unpin for CopyInto<'a, R, W> where CopyBufInto<'a, BufReader<R>, W>: Unpin {}

impl<'a, R: AsyncRead, W: ?Sized> CopyInto<'a, R, W> {
    unsafe_pinned!(inner: CopyBufInto<'a, BufReader<R>, W>);

    pub(super) fn new(reader: R, writer: &mut W) -> CopyInto<'_, R, W> {
        CopyInto {
            inner: CopyBufInto::new(BufReader::new(reader), writer),
        }
    }
}

impl<R: AsyncRead, W: AsyncWrite + Unpin + ?Sized> Future for CopyInto<'_, R, W> {
    type Output = io::Result<u64>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner().poll(cx)
    }
}
