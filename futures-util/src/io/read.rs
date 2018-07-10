use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use std::io;
use std::marker::Unpin;
use std::mem::PinMut;

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
#[derive(Debug)]
pub struct Read<'a, R: ?Sized + 'a> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

// Pinning is never projected to fields
impl<'a, R: ?Sized> Unpin for Read<'a, R> {}

impl<'a, R: AsyncRead + ?Sized> Read<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Read<'a, R> {
        Read { reader, buf }
    }
}

impl<'a, R: AsyncRead + ?Sized> Future for Read<'a, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let this = &mut *self;
        this.reader.poll_read(cx, this.buf)
    }
}
