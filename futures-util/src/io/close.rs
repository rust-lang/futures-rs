use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use futures_io::AsyncWrite;
use std::io;
use std::pin::Pin;

/// A future used to fully close an I/O object.
///
/// Created by the [`close`] function.
///
/// [`close`]: fn.close.html
#[derive(Debug)]
pub struct Close<'a, W: ?Sized> {
    writer: &'a mut W,
}

// Pin is never projected to fields
impl<W: ?Sized> Unpin for Close<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized> Close<'a, W> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Close { writer }
    }
}

impl<W: AsyncWrite + ?Sized> Future for Close<'_, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        self.writer.poll_close(waker)
    }
}
