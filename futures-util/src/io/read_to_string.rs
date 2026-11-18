use alloc::{string::String, vec::Vec};
use core::{mem, pin::Pin, str};
use std::io;

use futures_core::{
    future::Future,
    ready,
    task::{Context, Poll},
};
use futures_io::AsyncRead;

use super::read_to_end::read_to_end_internal;

/// Future for the [`read_to_string`](super::AsyncReadExt::read_to_string) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToString<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut String,
    bytes: Vec<u8>,
    start_len: usize,
    finished: bool,
}

impl<R: ?Sized + Unpin> Unpin for ReadToString<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadToString<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut String) -> Self {
        let start_len = buf.len();
        Self { reader, bytes: mem::take(buf).into_bytes(), buf, start_len, finished: false }
    }
}

fn read_to_string_internal<R: AsyncRead + ?Sized>(
    reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut String,
    bytes: &mut Vec<u8>,
    start_len: usize,
) -> Poll<io::Result<usize>> {
    let mut ret = ready!(read_to_end_internal(reader, cx, bytes, start_len));
    if str::from_utf8(&bytes[start_len..]).is_err() {
        bytes.truncate(start_len);
        if ret.is_ok() {
            ret = Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            ));
        }
    }
    debug_assert!(buf.is_empty());
    // Safety: `bytes` is valid UTF-8 because it was taken from a String
    // and the newly read bytes are either valid UTF-8 or have been removed.
    mem::swap(unsafe { buf.as_mut_vec() }, bytes);
    Poll::Ready(ret)
}

impl<A> Future for ReadToString<'_, A>
where
    A: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, buf, bytes, start_len, finished: _ } = &mut *self;
        let ret = ready!(read_to_string_internal(Pin::new(reader), cx, buf, bytes, *start_len));
        self.finished = true;
        Poll::Ready(ret)
    }
}

impl<R: ?Sized> Drop for ReadToString<'_, R> {
    fn drop(&mut self) {
        // restore old string contents
        if !self.finished {
            self.bytes.truncate(self.start_len);
            // Safety: `bytes` is valid UTF-8 because it was taken from a String
            // and the newly read bytes have been removed.
            mem::swap(unsafe { self.buf.as_mut_vec() }, &mut self.bytes);
        }
    }
}
