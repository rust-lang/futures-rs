use std::io::{self, Read};

use futures::{Task, Poll};
use futures::stream::Stream;

use Ready;

pub struct Repeat {
    byte: u8,
}

pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte: byte }
}

impl Stream for Repeat {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(Some(Ready::Read))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify();
    }
}

impl Read for Repeat {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        for slot in bytes.iter_mut() {
            *slot = self.byte;
        }
        Ok(bytes.len())
    }
}
