use std::io;
use std::marker::Unpin;
use std::mem::PinMut;
use std::vec::Vec;

use {Poll, Future, task};

use io::AsyncRead;

/// A future which can be used to easily read the entire contents of a stream
/// into a vector.
///
/// Created by the [`read_to_end`] function.
///
/// [`read_to_end`]: fn.read_to_end.html
#[derive(Debug)]
pub struct ReadToEnd<'a, A: ?Sized + 'a> {
    a: &'a mut A,
    buf: &'a mut Vec<u8>,
}

// We never project pinning to fields
impl<'a, A: ?Sized> Unpin for ReadToEnd<'a, A> {}

pub fn read_to_end<'a, A>(a: &'a mut A, buf: &'a mut Vec<u8>) -> ReadToEnd<'a, A>
    where A: AsyncRead + ?Sized,
{
    ReadToEnd { a, buf }
}

struct Guard<'a> { buf: &'a mut Vec<u8>, len: usize }

impl<'a> Drop for Guard<'a> {
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
fn read_to_end_internal<R: AsyncRead + ?Sized>(r: &mut R, cx: &mut task::Context, buf: &mut Vec<u8>)
    -> Poll<io::Result<()>>
{
    let mut g = Guard { len: buf.len(), buf: buf };
    let ret;
    loop {
        if g.len == g.buf.len() {
            unsafe {
                g.buf.reserve(32);
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);
                r.initializer().initialize(&mut g.buf[g.len..]);
            }
        }

        match r.poll_read(cx, &mut g.buf[g.len..]) {
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

impl<'a, A> Future for ReadToEnd<'a, A>
    where A: AsyncRead + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let this = &mut *self;
        read_to_end_internal(this.a, cx, this.buf)
    }
}
