use futures_core::future::Future;
use futures_core::task::{LocalWaker, Poll};
use std::io;
use std::marker::Unpin;
use std::pin::Pin;

use futures_io::AsyncWrite;

/// A future used to fully flush an I/O object.
///
/// Resolves to the underlying I/O object once the flush operation is complete.
///
/// Created by the [`flush`] function.
///
/// [`flush`]: fn.flush.html
#[derive(Debug)]
pub struct Flush<'a, W: ?Sized + 'a> {
    writer: &'a mut W,
}

// Pinning is never projected to fields
impl<W: ?Sized> Unpin for Flush<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized> Flush<'a, W> {
    pub(super) fn new(writer: &'a mut W) -> Self {
        Flush { writer }
    }
}

impl<W> Future for Flush<'_, W>
    where W: AsyncWrite + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        self.writer.poll_flush(lw)
    }
}
