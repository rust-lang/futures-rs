use std::io::{self, Write, Error, ErrorKind};
use std::cmp;

use futures::{Poll, Task};
use futures::stream::Stream;
use Ready;

/// An I/O object intended to mirror the `BufWriter` abstraction in the standard
/// library.
///
/// Implements a stream whose writes are amortized by buffering up small writes
/// into an internal buffer. This ensures that small writes don't all get issued
/// back to back but instead only a few larger writes are issued.
pub struct BufWriter<W> {
    inner: W,
    buf: Vec<u8>,
    flushing: bool,
}

impl<W> BufWriter<W> {
    /// Creates a new buffered writer with the default internal capacity.
    pub fn new(inner: W) -> BufWriter<W> {
        BufWriter::with_capacity(8 * 1024, inner)
    }

    /// Creates a new buffered writer with the specified capacity.
    pub fn with_capacity(cap: usize, inner: W) -> BufWriter<W> {
        BufWriter {
            inner: inner,
            buf: Vec::with_capacity(cap),
            flushing: false,
        }
    }

    /// Gets a shared reference to the internal object in this buffered writer.
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Gets a mutable reference to the internal object in this buffered
    /// writer.
    ///
    /// Note that care must be taken to not tamper with the I/O stream itself in
    /// terms of writing more bytes to it as bytes may otherwise then be
    /// received out of order.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Consumes this buffered writer, returning the underlying I/O object.
    ///
    /// This function will also consume the internal buffer of bytes, even if
    /// they haven't been written yet. It is recommended to `flush` this stream
    /// with the `Flush` combinator before performing this operation to ensure
    /// that all bytes are written.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> BufWriter<W> {
    fn flush_buf(&mut self) -> io::Result<()> {
        self.flushing = true;
        let mut written = 0;
        let len = self.buf.len();
        let mut ret = Ok(());
        while written < len {
            match self.inner.write(&self.buf[written..]) {
                Ok(0) => {
                    ret = Err(Error::new(ErrorKind::WriteZero,
                                         "failed to write the buffered data"));
                    break;
                }
                Ok(n) => written += n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    ret = Err(e);
                    break
                }
            }
        }
        if written > 0 {
            self.buf.drain(..written);
        }
        if written == len {
            self.flushing = false;
        }
        ret
    }
}

impl<A> Stream for BufWriter<A>
    where A: Stream<Item=Ready, Error=io::Error>,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        if !self.flushing && self.buf.len() < self.buf.capacity() {
            Poll::Ok(Some(Ready::Write))
        } else {
            self.inner.poll(task)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        // Notify immediately if we have some capacity, but also ask our
        // underlying stream for readiness so it'll be ready by the time that we
        // need to flush.
        if !self.flushing && self.buf.len() < self.buf.capacity() {
            task.notify()
        }
        self.inner.schedule(task)
    }
}

impl<W: Write> Write for BufWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.flushing || self.buf.len() + buf.len() > self.buf.capacity() {
            try!(self.flush_buf());
        }
        if buf.len() >= self.buf.capacity() {
            assert_eq!(self.buf.len(), 0);
            self.inner.write(buf)
        } else {
            let amt = cmp::min(buf.len(), self.buf.capacity());
            Write::write(&mut self.buf, &buf[..amt])
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        try!(self.flush_buf());
        self.inner.flush()
    }
}
