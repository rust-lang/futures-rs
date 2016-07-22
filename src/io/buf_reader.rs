use std::io::{self, Read};
use std::cmp;

use {Poll, Task};
use stream::Stream;
use io::{ReadStream, BufReadStream};

pub struct BufReader<R> {
    inner: R,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
}

impl<R> BufReader<R> {
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(8 * 1024, inner)
    }

    pub fn with_capacity(cap: usize, inner: R) -> BufReader<R> {
        BufReader {
            inner: inner,
            buf: vec![0; cap].into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }

    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<A> Stream for BufReader<A>
    where A: ReadStream,
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<()>, io::Error> {
        if self.pos < self.cap {
            Poll::Ok(Some(()))
        } else {
            self.inner.poll(task)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.pos < self.cap {
            task.notify()
        } else {
            self.inner.schedule(task)
        }
    }
}

impl<R: ReadStream> ReadStream for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.len() >= self.buf.len() {
            return self.inner.read(buf);
        }
        let nread = match try!(self.fill_buf()) {
            Some(mut rem) => try!(rem.read(buf)),
            None => return Ok(None),
        };
        self.consume(nread);
        Ok(Some(nread))
    }
}

impl<R: ReadStream> BufReadStream for BufReader<R> {
    fn fill_buf(&mut self) -> io::Result<Option<&[u8]>> {
        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        if self.pos == self.cap {
            self.cap = match try!(self.inner.read(&mut self.buf)) {
                Some(amt) => amt,
                None => return Ok(None)
            };
            self.pos = 0;
        }
        Ok(Some(&self.buf[self.pos..self.cap]))
    }

    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.cap);
    }
}
