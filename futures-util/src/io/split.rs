use crate::lock::BiLock;
use futures_core::task::{self, Poll};
use futures_io::{AsyncRead, AsyncWrite, IoVec};
use std::io;
use std::pin::Pin;

/// The readable half of an object returned from `AsyncRead::split`.
#[derive(Debug)]
pub struct ReadHalf<T> {
    handle: BiLock<T>,
}

/// The writable half of an object returned from `AsyncRead::split`.
#[derive(Debug)]
pub struct WriteHalf<T> {
    handle: BiLock<T>,
}

fn lock_and_then<T, U, E, F>(
    lock: &BiLock<T>,
    cx: &mut task::Context,
    f: F
) -> Poll<Result<U, E>>
    where F: FnOnce(&mut T, &mut task::Context) -> Poll<Result<U, E>>
{
    match lock.poll_lock(cx) {
        // Safety: the value behind the bilock used by `ReadHalf` and `WriteHalf` is never exposed
        // as a `Pin<&mut T>` anywhere other than here as a way to get to `&mut T`.
        Poll::Ready(mut l) => f(unsafe { Pin::get_mut_unchecked(l.as_pin_mut()) }, cx),
        Poll::Pending => Poll::Pending,
    }
}

pub fn split<T: AsyncRead + AsyncWrite>(t: T) -> (ReadHalf<T>, WriteHalf<T>) {
    let (a, b) = BiLock::new(t);
    (ReadHalf { handle: a }, WriteHalf { handle: b })
}

impl<R: AsyncRead> AsyncRead for ReadHalf<R> {
    fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_read(cx, buf))
    }

    fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVec])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_vectored_read(cx, vec))
    }
}

impl<W: AsyncWrite> AsyncWrite for WriteHalf<W> {
    fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_write(cx, buf))
    }

    fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_vectored_write(cx, vec))
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<io::Result<()>> {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_flush(cx))
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<io::Result<()>> {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_close(cx))
    }
}
