use std::io::{self, Read};

use futures::{Task, Poll};
use futures::stream::Stream;

use Ready;

/// An I/O object of an infinite stream of bytes that's always ready to read.
///
/// Created by the `repeat` function.
pub struct Repeat {
    byte: u8,
}

/// Creates an I/O combinator which is always ready for reading, and infinitely
/// yields a stream of bytes corresopnding to the value provided here.
///
/// This is similar to the standard library's `io::repeat` combinator as well.
pub fn repeat(byte: u8) -> Repeat {
    Repeat { byte: byte }
}

impl Stream for Repeat {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(Some(Ready::Read))
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
