use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::AsyncBufRead;
use std::io;
use std::mem;
use std::pin::Pin;
use std::str;
use super::read_until::read_until_internal;

/// Future for the [`read_line`](super::AsyncBufReadExt::read_line) method.
#[derive(Debug)]
pub struct ReadLine<'a, R: ?Sized + Unpin> {
    reader: &'a mut R,
    buf: &'a mut String,
    bytes: Vec<u8>,
    read: usize,
}

impl<R: ?Sized + Unpin> Unpin for ReadLine<'_, R> {}

impl<'a, R: AsyncBufRead + ?Sized + Unpin> ReadLine<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut String) -> Self {
        Self {
            reader,
            bytes: unsafe { mem::replace(buf.as_mut_vec(), Vec::new()) },
            buf,
            read: 0,
        }
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for ReadLine<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, buf, bytes, read } = &mut *self;
        let ret = ready!(read_until_internal(Pin::new(reader), b'\n', bytes, read, cx));
        if str::from_utf8(&bytes).is_err() {
            Poll::Ready(ret.and_then(|_| {
                Err(io::Error::new(io::ErrorKind::InvalidData,
                            "stream did not contain valid UTF-8"))
            }))
        } else {
            unsafe { mem::swap(buf.as_mut_vec(), bytes); }
            Poll::Ready(ret)
        }
    }
}
