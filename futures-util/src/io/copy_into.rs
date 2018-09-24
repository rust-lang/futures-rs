use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_io::{AsyncRead, AsyncWrite};
use std::boxed::Box;
use std::io;
use std::marker::Unpin;
use std::pin::Pin;

/// A future which will copy all data from a reader into a writer.
///
/// Created by the [`copy_into`] function, this future will resolve to the number of
/// bytes copied or an error if one happens.
///
/// [`copy_into`]: fn.copy_into.html
#[derive(Debug)]
pub struct CopyInto<'a, R: ?Sized + 'a, W: ?Sized + 'a> {
    reader: &'a mut R,
    read_done: bool,
    writer: &'a mut W,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Box<[u8]>,
}

// No projections of Pin<&mut CopyInto> into Pin<&mut Field> are ever done.
impl<R: ?Sized, W: ?Sized> Unpin for CopyInto<'_, R, W> {}

impl<'a, R: ?Sized, W: ?Sized> CopyInto<'a, R, W> {
    pub(super) fn new(reader: &'a mut R, writer: &'a mut W) -> Self {
        CopyInto {
            reader,
            read_done: false,
            writer,
            amt: 0,
            pos: 0,
            cap: 0,
            buf: Box::new([0; 2048]),
        }
    }
}

impl<R, W> Future for CopyInto<'_, R, W>
    where R: AsyncRead + ?Sized,
          W: AsyncWrite + ?Sized,
{
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let this = &mut *self;
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if this.pos == this.cap && !this.read_done {
                let n = try_ready!(this.reader.poll_read(cx, &mut this.buf));
                if n == 0 {
                    this.read_done = true;
                } else {
                    this.pos = 0;
                    this.cap = n;
                }
            }

            // If our buffer has some data, let's write it out!
            while this.pos < this.cap {
                let i = try_ready!(this.writer.poll_write(cx, &this.buf[this.pos..this.cap]));
                if i == 0 {
                    return Poll::Ready(Err(io::ErrorKind::WriteZero.into()))
                } else {
                    this.pos += i;
                    this.amt += i as u64;
                }
            }

            // If we've written al the data and we've seen EOF, flush out the
            // data and finish the transfer.
            // done with the entire transfer.
            if this.pos == this.cap && this.read_done {
                try_ready!(this.writer.poll_flush(cx));
                return Poll::Ready(Ok(this.amt));
            }
        }
    }
}
