use futures_io::{self as io, AsyncBufRead, AsyncRead, AsyncWrite};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
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
#[derive(Debug)]
pub struct Limited<Io> {
    io: Io,
    limit: usize,
}

impl<Io: Unpin> Unpin for Limited<Io> {}

impl<Io> Limited<Io> {
    unsafe_pinned!(io: Io);
    unsafe_unpinned!(limit: usize);

    pub(crate) fn new(io: Io, limit: usize) -> Limited<Io> {
        Limited { io, limit }
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
        self.io()
    }

    /// Consumes this adaptor returning the underlying I/O object.
    pub fn into_inner(self) -> Io {
        self.io
    }
}

impl<W: AsyncWrite> AsyncWrite for Limited<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let limit = *self.as_mut().limit();
        self.io().poll_write(cx, &buf[..cmp::min(limit, buf.len())])
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        self.io().poll_flush(cx)
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        self.io().poll_close(cx)
    }
}

impl<R: AsyncRead> AsyncRead for Limited<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let limit = cmp::min(*self.as_mut().limit(), buf.len());
        self.io().poll_read(cx, &mut buf[..limit])
    }
}

impl<R: AsyncBufRead> AsyncBufRead for Limited<R> {
    fn poll_fill_buf(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&[u8]>> {
        self.io().poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amount: usize) {
        self.io().consume(amount)
    }
}
