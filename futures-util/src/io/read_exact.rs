use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use std::io;
use std::marker::Unpin;
use std::mem;
use std::pin::Pin;

/// A future which can be used to easily read exactly enough bytes to fill
/// a buffer.
///
/// Created by the [`read_exact`] function.
///
/// [`read_exact`]: fn.read_exact.html
#[derive(Debug)]
pub struct ReadExact<'a, R: ?Sized + 'a> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<R: ?Sized> Unpin for ReadExact<'_, R> {}

impl<'a, R: AsyncRead + ?Sized> ReadExact<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        ReadExact { reader, buf }
    }
}

impl<R: AsyncRead + ?Sized> Future for ReadExact<'_, R> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let this = &mut *self;
        while !this.buf.is_empty() {
            let n = try_ready!(this.reader.poll_read(lw, this.buf));
            {
                let (_, rest) = mem::replace(&mut this.buf, &mut []).split_at_mut(n);
                this.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
            }
        }
        Poll::Ready(Ok(()))
    }
}
