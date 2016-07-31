use std::io::{self, Write};

use futures::{Task, Poll};
use futures::stream::Stream;

use Ready;

pub struct Sink {
    _inner: (),
}

pub fn sink() -> Sink {
    Sink { _inner: () }
}

impl Stream for Sink {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(Some(Ready::Write))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
