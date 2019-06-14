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
pub struct Limited<IO> {
    io: IO,
    limit: usize,
}

impl<IO: Unpin> Unpin for Limited<IO> {}

impl<IO> Limited<IO> {
    unsafe_pinned!(io: IO);
    unsafe_unpinned!(limit: usize);

    pub(crate) fn new(io: IO, limit: usize) -> Limited<IO> {
        Limited { io, limit }
    }

    /// Acquires a reference to the underlying I/O object that this adaptor is
    /// wrapping.
    pub fn get_ref(&self) -> &IO {
        &self.io
    }

    /// Acquires a mutable reference to the underlying I/O object that this
    /// adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut IO {
        &mut self.io
    }

    /// Acquires a pinned mutable reference to the underlying I/O object that
    /// this adaptor is wrapping.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut IO> {
        self.io()
    }

    /// Consumes this adaptor returning the underlying I/O object.
    pub fn into_inner(self) -> IO {
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
