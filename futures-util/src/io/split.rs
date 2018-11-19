use crate::lock::BiLock;
use futures_core::task::{Waker, Poll};
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
    waker: &Waker,
    f: F
) -> Poll<Result<U, E>>
    where F: FnOnce(&mut T, &Waker) -> Poll<Result<U, E>>
{
    match lock.poll_lock(waker) {
        // Safety: the value behind the bilock used by `ReadHalf` and `WriteHalf` is never exposed
        // as a `Pin<&mut T>` anywhere other than here as a way to get to `&mut T`.
        Poll::Ready(mut l) => f(unsafe { Pin::get_unchecked_mut(l.as_pin_mut()) }, waker),
        Poll::Pending => Poll::Pending,
    }
}

pub fn split<T: AsyncRead + AsyncWrite>(t: T) -> (ReadHalf<T>, WriteHalf<T>) {
    let (a, b) = BiLock::new(t);
    (ReadHalf { handle: a }, WriteHalf { handle: b })
}

impl<R: AsyncRead> AsyncRead for ReadHalf<R> {
    fn poll_read(&mut self, waker: &Waker, buf: &mut [u8])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, waker, |l, waker| l.poll_read(waker, buf))
    }

    fn poll_vectored_read(&mut self, waker: &Waker, vec: &mut [&mut IoVec])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, waker, |l, waker| l.poll_vectored_read(waker, vec))
    }
}

impl<W: AsyncWrite> AsyncWrite for WriteHalf<W> {
    fn poll_write(&mut self, waker: &Waker, buf: &[u8])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, waker, |l, waker| l.poll_write(waker, buf))
    }

    fn poll_vectored_write(&mut self, waker: &Waker, vec: &[&IoVec])
        -> Poll<io::Result<usize>>
    {
        lock_and_then(&self.handle, waker, |l, waker| l.poll_vectored_write(waker, vec))
    }

    fn poll_flush(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        lock_and_then(&self.handle, waker, |l, waker| l.poll_flush(waker))
    }

    fn poll_close(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        lock_and_then(&self.handle, waker, |l, waker| l.poll_close(waker))
    }
}
