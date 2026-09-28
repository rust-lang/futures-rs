use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_io::AsyncRead;
use std::io;
use std::pin::Pin;
use std::vec::Vec;

/// Future for the [`read_to_end`](super::AsyncReadExt::read_to_end) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToEnd<'a, R: ?Sized> {
    reader: &'a mut R,
    read_buf: ReadBuf<'a>,
}

impl<R: ?Sized + Unpin> Unpin for ReadToEnd<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadToEnd<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut Vec<u8>) -> Self {
        let read_buf = ReadBuf::new(buf);
        Self { reader, read_buf }
    }
}

// Owns the Vec and the read-state metadata, ensuring that
// `buf.len() <= initialized <= buf.capacity()`.
// Common functionality for `ReadToEnd` and `ReadToString`.
#[derive(Debug)]
pub(super) struct ReadBuf<'a> {
    buf: &'a mut Vec<u8>,
    original_len: usize,
    initialized: usize,
}

impl<'a> ReadBuf<'a> {
    #[inline]
    pub(super) fn new(buf: &'a mut Vec<u8>) -> Self {
        let len = buf.len();
        Self { buf, original_len: len, initialized: len }
    }

    #[inline]
    pub(super) fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    #[inline]
    pub(super) fn reset(&mut self) {
        self.buf.truncate(self.original_len);
    }
}

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than this if the reader has a very small
// amount of data to return.
//
// Because we're resizing the buffer with zeroes that will be overwritten by
// `poll_read`, we need to make sure to truncate them if something panics.
pub(super) fn read_to_end_internal<R: AsyncRead + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    rb: &mut ReadBuf<'_>,
) -> Poll<io::Result<usize>> {
    let mut g = Guard { len: rb.buf.len(), buf: &mut rb.buf };
    loop {
        if g.len == rb.initialized {
            // No pre-zeroed space remaining; need to grow.
            g.buf.reserve(32);
            let capacity = g.buf.capacity();
            g.buf.resize(capacity, 0);
            rb.initialized = capacity;
        }

        // Expose the pre-zeroed region [g.len..rb.initialized] to poll_read.
        // Guard::drop restores len to g.len on Pending, so this must run every iteration.
        // Safety: the bytes up to initialized have been filled with zeroes.
        unsafe { g.buf.set_len(rb.initialized) };
        let buf = &mut g.buf[g.len..];
        match ready!(rd.as_mut().poll_read(cx, buf)) {
            Ok(0) => return Poll::Ready(Ok(g.len - rb.original_len)),
            Ok(n) => {
                // We can't allow bogus values from read. If it is too large, the returned vec could have its length
                // set past its capacity, or if it overflows the vec could be shortened which could create an invalid
                // string if this is called via read_to_string.
                assert!(n <= buf.len());
                g.len += n;
            }
            Err(e) => return Poll::Ready(Err(e)),
        }
    }
}

impl<A> Future for ReadToEnd<'_, A>
where
    A: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        read_to_end_internal(Pin::new(&mut this.reader), cx, &mut this.read_buf)
    }
}
