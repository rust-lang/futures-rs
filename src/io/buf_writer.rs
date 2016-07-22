use std::io::{self, Write, Error, ErrorKind};
use std::cmp;

use {Poll, Task};
use stream::Stream;
use io::WriteStream;

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

impl<W: WriteStream> BufWriter<W> {
    fn flush_buf(&mut self) -> io::Result<bool> {
        self.flushing = true;
        let mut written = 0;
        let len = self.buf.len();
        let mut ret = Ok(true);
        while written < len {
            match self.inner.write(&self.buf[written..]) {
                Ok(Some(0)) => {
                    ret = Err(Error::new(ErrorKind::WriteZero,
                                         "failed to write the buffered data"));
                    break;
                }
                Ok(Some(n)) => written += n,
                Ok(None) => {
                    ret = Ok(false);
                    break
                }
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
    where A: WriteStream,
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<()>, io::Error> {
        if self.buf.len() < self.buf.capacity() {
            Poll::Ok(Some(()))
        } else {
            self.inner.poll(task)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        // Notify immediately if we have some capacity, but also ask our
        // underlying stream for readiness so it'll be ready by the time that we
        // need to flush.
        if self.buf.len() < self.buf.capacity() {
            task.notify()
        }
        self.inner.schedule(task)
    }
}

impl<W: WriteStream> WriteStream for BufWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
        if self.flushing || self.buf.len() + buf.len() > self.buf.capacity() {
            if !try!(self.flush_buf()) {
                return Ok(None)
            }
        }
        if buf.len() >= self.buf.capacity() {
            self.inner.write(buf)
        } else {
            let amt = cmp::min(buf.len(), self.buf.capacity());
            Write::write(&mut self.buf, &buf[..amt]).map(Some)
        }
    }

    fn flush(&mut self) -> io::Result<bool> {
        Ok(try!(self.flush_buf()) && try!(self.inner.flush()))
    }
}
