use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use futures_io::AsyncWrite;
use std::io;
use std::mem;
use std::pin::Pin;

/// A future used to write the entire contents of some data to a stream.
///
/// This is created by the [`write_all`] top-level method.
///
/// [`write_all`]: fn.write_all.html
#[derive(Debug)]
pub struct WriteAll<'a, W: ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
}

// Pinning is never projected to fields
impl<W: ?Sized> Unpin for WriteAll<'_, W> {}

impl<'a, W: AsyncWrite + ?Sized> WriteAll<'a, W> {
    pub(super) fn new(writer: &'a mut W, buf: &'a [u8]) -> Self {
        WriteAll { writer, buf }
    }
}

impl<W: AsyncWrite + ?Sized> Future for WriteAll<'_, W> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<io::Result<()>> {
        let this = &mut *self;
        while !this.buf.is_empty() {
            let n = try_ready!(this.writer.poll_write(waker, this.buf));
            {
                let (_, rest) = mem::replace(&mut this.buf, &[]).split_at(n);
                this.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()))
            }
        }

        Poll::Ready(Ok(()))
    }
}
