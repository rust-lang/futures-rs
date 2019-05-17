use futures_io::{self as io, AsyncWrite};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::{
    cmp,
    marker::Unpin,
    pin::Pin,
    task::{Context, Poll},
};

/// Writer for the [`limited_write`](super::AsyncWriteTestExt::limited_write) method.
#[derive(Debug)]
pub struct LimitedWrite<W: AsyncWrite> {
    writer: W,
    limit: usize,
}

impl<W: AsyncWrite + Unpin> Unpin for LimitedWrite<W> {}

impl<W: AsyncWrite> LimitedWrite<W> {
    unsafe_pinned!(writer: W);
    unsafe_unpinned!(limit: usize);

    pub(crate) fn new(writer: W, limit: usize) -> LimitedWrite<W> {
        LimitedWrite { writer, limit }
    }

    /// Acquires a reference to the underlying writer that this adaptor is wrapping.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Acquires a mutable reference to the underlying writer that this adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Acquires a pinned mutable reference to the underlying writer that this adaptor is wrapping.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut W> {
        self.writer()
    }

    /// Consumes this adaptor returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: AsyncWrite> AsyncWrite for LimitedWrite<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let limit = *self.as_mut().limit();
        self.writer()
            .poll_write(cx, &buf[..cmp::min(limit, buf.len())])
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        self.writer().poll_flush(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        self.writer().poll_close(cx)
    }
}
