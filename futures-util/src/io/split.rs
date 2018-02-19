use std::io;

use {Async, Poll, task};
use lock::BiLock;

use futures_io::{AsyncRead, AsyncWrite, Error, IoVec};

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

fn lock_and_then<T, U, E, F>(lock: &BiLock<T>, cx: &mut task::Context, f: F) -> Result<Async<U>, E>
    where F: FnOnce(&mut T, &mut task::Context) -> Result<Async<U>, E>
{
    match lock.poll_lock(cx) {
        Async::Ready(ref mut l) => f(l, cx),
        Async::Pending => Ok(Async::Pending),
    }
}

pub fn split<T: AsyncRead + AsyncWrite>(t: T) -> (ReadHalf<T>, WriteHalf<T>) {
    let (a, b) = BiLock::new(t);
    (ReadHalf { handle: a }, WriteHalf { handle: b })
}

impl<T: AsyncRead> AsyncRead for ReadHalf<T> {
    fn poll_read(&mut self, buf: &mut [u8], cx: &mut task::Context)
        -> Poll<usize, io::Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_read(buf, cx))
    }

    fn poll_vectored_read(&mut self, vec: &mut [&mut IoVec], cx: &mut task::Context)
        -> Poll<usize, io::Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_vectored_read(vec, cx))
    }
}

impl<T: AsyncWrite> AsyncWrite for WriteHalf<T> {
    fn poll_write(&mut self, buf: &[u8], cx: &mut task::Context)
        -> Poll<usize, Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_write(buf, cx))
    }

    fn poll_vectored_write(&mut self, vec: &[&IoVec], cx: &mut task::Context)
        -> Poll<usize, Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_vectored_write(vec, cx))
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_flush(cx))
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_close(cx))
    }
}
