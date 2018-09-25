use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_io::AsyncRead;
use std::io;
use std::marker::Unpin;
use std::pin::Pin;
use std::vec::Vec;

/// A future which can be used to easily read the entire contents of a stream
/// into a vector.
///
/// Created by the [`read_to_end`] function.
///
/// [`read_to_end`]: fn.read_to_end.html
#[derive(Debug)]
pub struct ReadToEnd<'a, R: ?Sized + 'a> {
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
}

// We never project pinning to fields
impl<R: ?Sized> Unpin for ReadToEnd<'_, R> {}

impl<'a, R: AsyncRead + ?Sized> ReadToEnd<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut Vec<u8>) -> Self {
        ReadToEnd { reader, buf }
    }
}

struct Guard<'a> { buf: &'a mut Vec<u8>, len: usize }

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe { self.buf.set_len(self.len); }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than this if the reader has a very small
// amount of data to return.
//
// Because we're extending the buffer with uninitialized data for trusted
// readers, we need to make sure to truncate that if any of this panics.
fn read_to_end_internal<R: AsyncRead + ?Sized>(
    rd: &mut R,
    lw: &LocalWaker,
    buf: &mut Vec<u8>,
) -> Poll<io::Result<()>> {
    let mut g = Guard { len: buf.len(), buf };
    let ret;
    loop {
        if g.len == g.buf.len() {
            unsafe {
                g.buf.reserve(32);
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);
                rd.initializer().initialize(&mut g.buf[g.len..]);
            }
        }

        match rd.poll_read(lw, &mut g.buf[g.len..]) {
            Poll::Ready(Ok(0)) => {
                ret = Poll::Ready(Ok(()));
                break;
            }
            Poll::Ready(Ok(n)) => g.len += n,
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(e)) => {
                ret = Poll::Ready(Err(e));
                break;
            }
        }
    }

    ret
}

impl<A> Future for ReadToEnd<'_, A>
    where A: AsyncRead + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let this = &mut *self;
        read_to_end_internal(this.reader, lw, this.buf)
    }
}
