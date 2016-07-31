use std::io::{self, Write, Error, ErrorKind};
use std::cmp;

use futures::{Poll, Task};
use futures::stream::Stream;
use Ready;

pub struct BufWriter<W> {
    inner: W,
    buf: Vec<u8>,
    flushing: bool,
}

impl<W> BufWriter<W> {
    pub fn new(inner: W) -> BufWriter<W> {
        BufWriter::with_capacity(8 * 1024, inner)
    }

    pub fn with_capacity(cap: usize, inner: W) -> BufWriter<W> {
        BufWriter {
            inner: inner,
            buf: Vec::with_capacity(cap),
            flushing: false,
        }
    }

    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

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
