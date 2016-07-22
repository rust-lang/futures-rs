use std::io;

use {Task, Poll};
use stream::Stream;
use io::ReadStream;

pub struct Repeat {
    byte: u8,
}

pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte: byte }
}

impl Stream for Repeat {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<()>, io::Error> {
        Poll::Ok(Some(()))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}

impl ReadStream for Repeat {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        for slot in buf.iter_mut() {
            *slot = self.byte;
        }
        Ok(Some(buf.len()))
    }
}
