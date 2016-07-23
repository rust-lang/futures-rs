// TODO: upstream access to the internal buffer and use libstd BufReader

use std::io::{self, Read, BufRead};
use std::cmp;

use {Poll, Task};
use stream::Stream;
use io::Ready;

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

impl<R> Stream for BufReader<R>
    where R: Stream<Item=Ready, Error=io::Error>,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        if self.pos < self.cap {
            Poll::Ok(Some(Ready::Read))
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

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.len() >= self.buf.len() {
            return self.inner.read(buf);
        }
        let nread = {
            let mut rem = try!(self.fill_buf());
            try!(rem.read(buf))
        };
        self.consume(nread);
        Ok(nread)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        if self.pos == self.cap {
            self.cap = try!(self.inner.read(&mut self.buf));
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.cap);
    }
}
