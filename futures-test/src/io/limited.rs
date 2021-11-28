use futures_io::{self as io, AsyncBufRead, AsyncRead, AsyncWrite};
use pin_project::pin_project;
use std::{
    cmp,
    pin::Pin,
    task::{Context, Poll},
};

/// I/O wrapper that limits the number of bytes written or read on each call.
///
/// See the [`limited`] and [`limited_write`] methods.
///
/// [`limited`]: super::AsyncReadTestExt::limited
/// [`limited_write`]: super::AsyncWriteTestExt::limited_write
#[pin_project]
#[derive(Debug)]
pub struct Limited<Io> {
    #[pin]
    io: Io,
    limit: usize,
}

impl<Io> Limited<Io> {
    pub(crate) fn new(io: Io, limit: usize) -> Self {
        Self { io, limit }
    }

    /// Acquires a reference to the underlying I/O object that this adaptor is
    /// wrapping.
    pub fn get_ref(&self) -> &Io {
        &self.io
    }

    /// Acquires a mutable reference to the underlying I/O object that this
    /// adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut Io {
        &mut self.io
    }

    /// Acquires a pinned mutable reference to the underlying I/O object that
    /// this adaptor is wrapping.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Io> {
        self.project().io
    }

    /// Consumes this adaptor returning the underlying I/O object.
    pub fn into_inner(self) -> Io {
        self.io
    }
}

impl<W: AsyncWrite> AsyncWrite for Limited<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.io.poll_write(cx, &buf[..cmp::min(*this.limit, buf.len())])
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().io.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().io.poll_close(cx)
    }
}

impl<R: AsyncRead> AsyncRead for Limited<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let limit = cmp::min(*this.limit, buf.len());
        this.io.poll_read(cx, &mut buf[..limit])
    }
}

impl<R: AsyncBufRead> AsyncBufRead for Limited<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.project().io.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amount: usize) {
        self.project().io.consume(amount)
    }
}
