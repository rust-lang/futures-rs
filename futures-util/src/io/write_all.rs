use std::io;
use std::marker::Unpin;
use std::mem::{self, PinMut};

use crate::{Poll, Future, task};

use futures_io::AsyncWrite;

/// A future used to write the entire contents of some data to a stream.
///
/// This is created by the [`write_all`] top-level method.
///
/// [`write_all`]: fn.write_all.html
#[derive(Debug)]
pub struct WriteAll<'a, A: ?Sized + 'a> {
    a: &'a mut A,
    buf: &'a [u8],
}

// Pinning is never projected to fields
impl<'a, A: ?Sized> Unpin for WriteAll<'a, A> {}

pub fn write_all<'a, A>(a: &'a mut A, buf: &'a [u8]) -> WriteAll<'a, A>
    where A: AsyncWrite + ?Sized,
{
    WriteAll { a, buf }
}

fn zero_write() -> io::Error {
    io::Error::new(io::ErrorKind::WriteZero, "zero-length write")
}

impl<'a, A> Future for WriteAll<'a, A>
    where A: AsyncWrite + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        let this = &mut *self;
        while !this.buf.is_empty() {
            let n = try_ready!(this.a.poll_write(cx, this.buf));
            {
                let (rest, _) = mem::replace(&mut this.buf, &[]).split_at(n);
                this.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(zero_write()))
            }
        }

        Poll::Ready(Ok(()))
    }
}
