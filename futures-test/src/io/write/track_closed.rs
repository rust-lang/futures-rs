use futures_io::AsyncWrite;
use std::{
    io::{Error, ErrorKind, IoSlice, Result},
    pin::Pin,
    task::{Context, Poll},
};

/// I/O wrapper that tracks whether it has been closed.
///
/// See the [`track_closed`] method for more details.
///
/// [`track_closed`]: super::AsyncWriteTestExt::limited_write
#[pin_project::pin_project]
#[derive(Debug)]
pub struct TrackClosed<W> {
    #[pin]
    inner: W,
    closed: bool,
}

impl<W> TrackClosed<W> {
    pub(crate) fn new(inner: W) -> TrackClosed<W> {
        TrackClosed {
            inner,
            closed: false,
        }
    }

    /// Check whether this stream has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Acquires a reference to the underlying I/O object that this adaptor is
    /// wrapping.
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Acquires a mutable reference to the underlying I/O object that this
    /// adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Acquires a pinned mutable reference to the underlying I/O object that
    /// this adaptor is wrapping.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().inner
    }

    /// Consumes this adaptor returning the underlying I/O object.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: AsyncWrite> AsyncWrite for TrackClosed<W> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        if self.is_closed() {
            return Poll::Ready(Err(Error::new(
                ErrorKind::Other,
                "Attempted to write after stream was closed",
            )));
        }
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        if self.is_closed() {
            return Poll::Ready(Err(Error::new(
                ErrorKind::Other,
                "Attempted to flush after stream was closed",
            )));
        }
        assert!(!self.is_closed());
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        if self.is_closed() {
            return Poll::Ready(Err(Error::new(
                ErrorKind::Other,
                "Attempted to close after stream was closed",
            )));
        }
        let this = self.project();
        match this.inner.poll_close(cx) {
            Poll::Ready(Ok(())) => {
                *this.closed = true;
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        if self.is_closed() {
            return Poll::Ready(Err(Error::new(
                ErrorKind::Other,
                "Attempted to write after stream was closed",
            )));
        }
        self.project().inner.poll_write_vectored(cx, bufs)
    }
}
