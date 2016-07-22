use std::io;

use {Task, Poll};
use stream::Stream;
use io::WriteStream;

pub struct Sink(());

pub fn sink() -> Sink {
    Sink(())
}

impl Stream for Sink {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<()>, io::Error> {
        Poll::Ok(Some(()))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}

impl WriteStream for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
        Ok(Some(buf.len()))
    }

    fn flush(&mut self) -> io::Result<bool> {
        Ok(true)
    }
}

