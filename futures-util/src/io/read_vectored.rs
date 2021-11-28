use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use std::io::{self, IoSliceMut};
use std::pin::Pin;

/// Future for the [`read_vectored`](super::AsyncReadExt::read_vectored) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadVectored<'a, 'b, R: ?Sized> {
    reader: &'a mut R,
    bufs: &'a mut [IoSliceMut<'b>],
}

impl<R: ?Sized + Unpin> Unpin for ReadVectored<'_, '_, R> {}

impl<'a, 'b, R: AsyncRead + ?Sized + Unpin> ReadVectored<'a, 'b, R> {
    pub(super) fn new(reader: &'a mut R, bufs: &'a mut [IoSliceMut<'b>]) -> Self {
        Self { reader, bufs }
    }
}

impl<R: AsyncRead + ?Sized + Unpin> Future for ReadVectored<'_, '_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.reader).poll_read_vectored(cx, this.bufs)
    }
}
