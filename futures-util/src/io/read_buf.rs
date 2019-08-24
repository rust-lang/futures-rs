use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncRead, BufMut};
use std::io;
use std::pin::Pin;

/// Future for the [`read_buf`](super::AsyncReadExt::read_buf) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadBuf<'a, R: ?Sized + Unpin, B> {
    reader: &'a mut R,
    buf: &'a mut B,
}

impl<R: ?Sized + Unpin, B> Unpin for ReadBuf<'_, R, B> {}

impl<'a, R: AsyncRead + ?Sized + Unpin, B: BufMut> ReadBuf<'a, R, B> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut B) -> Self {
        Self { reader, buf }
    }
}

impl<R: AsyncRead + ?Sized + Unpin, B: BufMut> Future for ReadBuf<'_, R, B> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.reader).poll_read_buf(cx, this.buf)
    }
}
