use std::io::{self, Read};
use std::cmp;

use {Poll, Task};
use stream::Stream;
use io::Ready;

pub struct Take<A> {
    a: A,
    left: u64,
}

pub fn take<A>(a: A, amt: u64) -> Take<A>
    where A: Stream<Item=Ready, Error=io::Error> + Read,
{
    Take {
        a: a,
        left: amt,
    }
}

impl<A> Stream for Take<A>
    where A: Stream<Item=Ready, Error=io::Error> + Read,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        if self.left == 0 {
            Poll::Ok(None)
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

impl<A> Read for Take<A>
    where A: Stream<Item=Ready, Error=io::Error> + Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Ok(0)
        }

        let amt = cmp::min(buf.len() as u64, self.left) as usize;
        let n = try!(self.a.read(&mut buf[..amt]));
        self.left -= n as u64;
        Ok(n)
    }
}
