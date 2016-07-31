use std::io::{self, Read};

use futures::{Task, Poll};
use futures::stream::Stream;

use Ready;

pub struct Empty {
    _inner: (),
}

pub fn empty() -> Empty {
    Empty { _inner: () }
}

impl Stream for Empty {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(Some(Ready::Read))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify();
    }
}

impl Read for Empty {
    fn read(&mut self, _bytes: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}
