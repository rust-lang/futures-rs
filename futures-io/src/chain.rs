use std::io;

use futures::{Poll, Task};
use futures::stream::Stream;

use {Ready, ReadTask};

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
    where A: ReadTask,
          B: ReadTask,
{
    Chain {
        a: a,
        b: b,
        first: true,
    }
}

impl<A, B> Stream for Chain<A, B>
    where A: ReadTask,
          B: ReadTask,
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
}

impl<A, B> ReadTask for Chain<A, B>
    where A: ReadTask,
          B: ReadTask,
{
    fn read(&mut self, task: &mut Task, buf: &mut [u8]) -> io::Result<usize> {
        if self.first {
            match self.a.read(task, buf) {
                Ok(0) => self.first = false,
                other => return other,
            }
        }
        self.b.read(task, buf)
    }

    fn read_to_end(&mut self,
                   task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut amt = 0;
        if self.first {
            match self.a.read_to_end(task, buf) {
                Ok(n) => {
                    self.first = false;
                    amt += n;
                }
                other => return other,
            }
        }
        self.b.read_to_end(task, buf).map(|n| n + amt)
    }
}
