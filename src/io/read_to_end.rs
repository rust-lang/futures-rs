use std::io;
use std::iter;
use std::mem;

use {Poll, Task, Future};
use io::ReadStream;

pub struct ReadToEnd<A> {
    a: A,
    buf: Vec<u8>,
}

pub fn new<A>(a: A, buf: Vec<u8>) -> ReadToEnd<A>
    where A: ReadStream,
{
    ReadToEnd {
        a: a,
        buf: buf,
    }
}

impl<A> Future for ReadToEnd<A>
    where A: ReadStream,
{
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Vec<u8>, io::Error> {
        if let Err(e) = try_poll!(self.a.poll(task)) {
            return Poll::Err(e)
        }
        loop {
            let start = self.buf.len();
            if self.buf.capacity() == start {
                self.buf.reserve(1);
            }
            let end = self.buf.capacity();
            // TODO: be smarter about extending with 0, don't keep doing so for
            //       regions that are already zero'd
            self.buf.extend(iter::repeat(0).take(end - start));
            match self.a.read(&mut self.buf[start..]) {
                Ok(Some(0)) => {
                    self.buf.truncate(start);
                    return Poll::Ok(mem::replace(&mut self.buf, Vec::new()))
                }
                Ok(Some(amt)) => self.buf.truncate(start + amt),
                Ok(None) => {
                    self.buf.truncate(start);
                    return Poll::NotReady
                }
                Err(e) => {
                    self.buf.truncate(start);
                    return Poll::Err(e)
                }
            }
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        self.a.schedule(task)
    }
}
