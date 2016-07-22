use std::io;
use std::cmp;

use {Poll, Task};
use stream::Stream;
use io::ReadStream;

pub struct Limit<A> {
    a: A,
    left: u64,
}

pub fn new<A>(a: A, amt: u64) -> Limit<A>
    where A: ReadStream,
{
    Limit {
        a: a,
        left: amt,
    }
}

impl<A> Stream for Limit<A>
    where A: ReadStream,
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<()>, io::Error> {
        if self.left == 0 {
            Poll::Ok(Some(()))
        } else {
            self.a.poll(task)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.left == 0 {
            task.notify()
        } else {
            self.a.schedule(task)
        }
    }
}

impl<A> ReadStream for Limit<A>
    where A: ReadStream,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        if self.left == 0 {
            return Ok(Some(0))
        }

        let amt = cmp::min(buf.len() as u64, self.left) as usize;
        let ret = try!(self.a.read(&mut buf[..amt]));
        if let Some(n) = ret {
            self.left -= n as u64;
        }
        Ok(ret)
    }
}
