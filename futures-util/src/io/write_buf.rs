use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncWrite, Buf};
use std::io;
use std::pin::Pin;

/// Future for the [`write_buf`](super::AsyncWriteExt::write_buf) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteBuf<'a, W: ?Sized + Unpin, B> {
    writer: &'a mut W,
    buf: &'a mut B,
}

impl<W: ?Sized + Unpin, B> Unpin for WriteBuf<'_, W, B> {}

impl<'a, W: AsyncWrite + ?Sized + Unpin, B: Buf> WriteBuf<'a, W, B> {
    pub(super) fn new(writer: &'a mut W, buf: &'a mut B) -> Self {
        Self { writer, buf }
    }
}

impl<W: AsyncWrite + ?Sized + Unpin, B: Buf> Future for WriteBuf<'_, W, B> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.writer).poll_write_buf(cx, this.buf)
    }
}
