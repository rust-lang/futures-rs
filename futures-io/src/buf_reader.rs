// TODO: upstream access to the internal buffer and use libstd BufReader

use std::io::{self, Read};
use std::cmp;

use futures::{Poll, Task};
use futures::stream::Stream;
use {Ready, ReadTask, BufReadTask};

/// An I/O object which amortizes a number of `read` operations by reading many
/// bytes at once into an internal buffer.
///
/// This object is intended to mirror the `BufReader` abstraction in the
/// standard library and simply implements a stream of readiness on top of it as
/// well.
pub struct BufReader<R> {
    inner: R,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
}

impl<R> BufReader<R> {
    /// Creates a new buffered reader wrapping the specified stream with the
    /// default capacity.
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(8 * 1024, inner)
    }

    /// Creates a new buffered reader with the specified capacity.
    pub fn with_capacity(cap: usize, inner: R) -> BufReader<R> {
        BufReader {
            inner: inner,
            buf: vec![0; cap].into_boxed_slice(),
            pos: 0,
            cap: 0,
        }
    }

    /// Gets a shared reference to the internal object in this buffered reader.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the internal object in this buffered
    /// reader.
    ///
    /// Note that care must be taken to not tamper with the I/O stream itself in
    /// terms of reading bytes from it as bytes may otherwise then be received
    /// out of order.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Consumes this object, returning the underlying I/O object.
    ///
    /// Note that this discards the internal buffer of bytes, so it is
    /// recommended to read this reader until EOF if that is not desired.
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

impl<R: ReadTask> ReadTask for BufReader<R> {
    fn read(&mut self, task: &mut Task, buf: &mut [u8]) -> io::Result<usize> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        if self.pos == self.cap && buf.len() >= self.buf.len() {
            return self.inner.read(task, buf);
        }
        let nread = {
            let mut rem = try!(self.fill_buf(task));
            try!(rem.read(buf))
        };
        self.consume(task, nread);
        Ok(nread)
    }

    fn read_to_end(&mut self,
                   task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize> {
        let buffered = self.cap - self.pos;
        if self.pos != self.cap {
            buf.extend_from_slice(&self.buf[self.pos..self.cap]);
            self.pos = self.cap;
        }
        self.inner.read_to_end(task, buf).map(|n| n + buffered)
    }
}

impl<R: ReadTask> BufReadTask for BufReader<R> {
    fn fill_buf(&mut self, task: &mut Task) -> io::Result<&[u8]> {
        // If we've reached the end of our internal buffer then we need to fetch
        // some more data from the underlying reader.
        if self.pos == self.cap {
            self.cap = try!(self.inner.read(task, &mut self.buf));
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    fn consume(&mut self, _task: &mut Task, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.cap);
    }
}
