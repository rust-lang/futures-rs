use futures_io::AsyncWrite;
use futures_sink::Sink;
use std::{
    io::{self, IoSlice},
    pin::Pin,
    task::{Context, Poll},
};

/// Async wrapper that tracks whether it has been closed.
///
/// See the `track_closed` methods on:
/// * [`SinkTestExt`](crate::sink::SinkTestExt::track_closed)
/// * [`AsyncWriteTestExt`](crate::io::AsyncWriteTestExt::track_closed)
#[pin_project::pin_project]
#[derive(Debug)]
pub struct TrackClosed<T> {
    #[pin]
    inner: T,
    closed: bool,
}

impl<T> TrackClosed<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self { inner, closed: false }
    }

    /// Check whether this object has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Acquires a reference to the underlying object that this adaptor is
    /// wrapping.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Acquires a mutable reference to the underlying object that this
    /// adaptor is wrapping.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Acquires a pinned mutable reference to the underlying object that
    /// this adaptor is wrapping.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        self.project().inner
    }

    /// Consumes this adaptor returning the underlying object.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: AsyncWrite> AsyncWrite for TrackClosed<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.is_closed() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Other,
                "Attempted to write after stream was closed",
            )));
        }
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.is_closed() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Other,
                "Attempted to flush after stream was closed",
            )));
        }
        assert!(!self.is_closed());
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.is_closed() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Other,
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
    ) -> Poll<io::Result<usize>> {
        if self.is_closed() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Other,
                "Attempted to write after stream was closed",
            )));
        }
        self.project().inner.poll_write_vectored(cx, bufs)
    }
}

impl<Item, T: Sink<Item>> Sink<Item> for TrackClosed<T> {
    type Error = T::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        assert!(!self.is_closed());
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        assert!(!self.is_closed());
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        assert!(!self.is_closed());
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        assert!(!self.is_closed());
        let this = self.project();
        match this.inner.poll_close(cx) {
            Poll::Ready(Ok(())) => {
                *this.closed = true;
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
