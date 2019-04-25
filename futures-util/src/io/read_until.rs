use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::AsyncBufRead;
use std::io;
use std::pin::Pin;

/// Future for the [`read_until`](super::AsyncBufReadExt::read_until) method.
#[derive(Debug)]
pub struct ReadUntil<'a, R: ?Sized + Unpin> {
    reader: &'a mut R,
    byte: u8,
    buf: &'a mut Vec<u8>,
}

impl<R: ?Sized + Unpin> Unpin for ReadUntil<'_, R> {}

impl<'a, R: AsyncBufRead + ?Sized + Unpin> ReadUntil<'a, R> {
    pub(super) fn new(reader: &'a mut R, byte: u8, buf: &'a mut Vec<u8>) -> Self {
        Self { reader, byte, buf }
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for ReadUntil<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        let mut read = 0;
        loop {
            let (done, used) = {
                let available = try_ready!(Pin::new(&mut this.reader).poll_fill_buf(cx));
                if let Some(i) = memchr::memchr(this.byte, available) {
                    this.buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                } else {
                    this.buf.extend_from_slice(available);
                    (false, available.len())
                }
            };
            Pin::new(&mut this.reader).consume(used);
            read += used;
            if done || used == 0 {
                return Poll::Ready(Ok(read));
            }
        }
    }
}
