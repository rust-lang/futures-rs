use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use std::io;
use std::pin::Pin;

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
#[derive(Debug)]
pub struct Read<'a, R: ?Sized + Unpin> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<R: ?Sized + Unpin> Unpin for Read<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> Read<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        Read { reader, buf }
    }
}

impl<R: AsyncRead + ?Sized + Unpin> Future for Read<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let this = &mut *self;
        Pin::new(&mut this.reader).poll_read(waker, this.buf)
    }
}
