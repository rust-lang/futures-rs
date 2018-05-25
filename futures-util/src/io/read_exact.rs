use std::io;
use std::marker::Unpin;
use std::mem::{self, PinMut};

use {Poll, Future, task};

use io::AsyncRead;

/// A future which can be used to easily read exactly enough bytes to fill
/// a buffer.
///
/// Created by the [`read_exact`] function.
///
/// [`read_exact`]: fn.read_exact.html
#[derive(Debug)]
pub struct ReadExact<'a, A: ?Sized + 'a> {
    a: &'a mut A,
    buf: &'a mut [u8],
}

impl<'a, A: ?Sized> Unpin for ReadExact<'a, A> {}

pub fn read_exact<'a, A>(a: &'a mut A, buf: &'a mut [u8]) -> ReadExact<'a, A>
    where A: AsyncRead + ?Sized,
{
    ReadExact { a, buf }
}

fn eof() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "early eof")
}

impl<'a, A> Future for ReadExact<'a, A>
    where A: AsyncRead + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let this = &mut *self;
        while this.buf.len() > 0 {
            let n = {
                match this.a.poll_read(cx, this.buf) {
                    Poll::Ready(Ok(n)) => n,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            };
            {
                let (rest, _) = mem::replace(&mut this.buf, &mut []).split_at_mut(n);
                this.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(eof()))
            }
        }
        Poll::Ready(Ok(()))
    }
}
