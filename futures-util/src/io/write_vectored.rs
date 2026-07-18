use core::pin::Pin;
use std::io::{self, IoSlice};

use futures_core::{
    future::Future,
    task::{Context, Poll},
};

use crate::io::AsyncWrite;

/// Future for the [`write_vectored`](super::AsyncWriteExt::write_vectored) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteVectored<'a, 'b, W: ?Sized> {
    writer: &'a mut W,
    bufs: &'a [IoSlice<'b>],
}

impl<W: ?Sized + Unpin> Unpin for WriteVectored<'_, '_, W> {}

impl<'a, 'b, W: AsyncWrite + ?Sized + Unpin> WriteVectored<'a, 'b, W> {
    pub(super) fn new(writer: &'a mut W, bufs: &'a [IoSlice<'b>]) -> Self {
        Self { writer, bufs }
    }
}

impl<W: AsyncWrite + ?Sized + Unpin> Future for WriteVectored<'_, '_, W> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.writer).poll_write_vectored(cx, this.bufs)
    }
}
