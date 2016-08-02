use std::io::{self, Read};

use futures::{Task, Poll};
use futures::stream::Stream;

use Ready;

/// An I/O combinator which is always ready for a read and is always at EOF.
///
/// Created by the `empty` function.
pub struct Empty {
    _inner: (),
}

/// An I/O object which is always at EOF and ready for a read.
///
/// Similar to the `std::io::empty` combinator.
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
