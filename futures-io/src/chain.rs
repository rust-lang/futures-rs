use std::io::{self, Read};

use futures::{Poll, Task};
use futures::stream::Stream;

use Ready;

/// An I/O combinator which will read all bytes from one stream and then the
/// next.
///
/// Created by the `chain` function.
pub struct Chain<A, B> {
    a: A,
    b: B,
    first: bool,
}

/// Chains one I/O stream onto another.
///
/// Creates a new I/O object which will read all the bytes from `a` and then all
/// the bytes from `b` once it's hit EOF.
pub fn chain<A, B>(a: A, b: B) -> Chain<A, B>
    where A: Stream<Item=Ready, Error=io::Error> + Read,
          B: Stream<Item=Ready, Error=io::Error> + Read,
{
    Chain {
        a: a,
        b: b,
        first: true,
    }
}

impl<A, B> Stream for Chain<A, B>
    where A: Stream<Item=Ready, Error=io::Error> + Read,
          B: Stream<Item=Ready, Error=io::Error> + Read,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        if self.first {
            self.a.poll(task)
        } else {
            self.b.poll(task)
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.first {
            self.a.schedule(task)
        } else {
            self.b.schedule(task)
        }
    }
}

impl<A, B> Read for Chain<A, B>
    where A: Stream<Item=Ready, Error=io::Error> + Read,
          B: Stream<Item=Ready, Error=io::Error> + Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.first {
            match self.a.read(buf) {
                Ok(0) => self.first = false,
                other => return other,
            }
        }
        self.b.read(buf)
    }
}
