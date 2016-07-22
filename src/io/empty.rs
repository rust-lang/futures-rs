use std::io;

use {Task, Poll};
use stream::Stream;
use io::{ReadStream, BufReadStream};

pub struct Empty(());

pub fn empty() -> Empty {
    Empty(())
}

impl Stream for Empty {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<()>, io::Error> {
        Poll::Ok(Some(()))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}

impl ReadStream for Empty {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<Option<usize>> {
        Ok(Some(0))
    }
}

impl BufReadStream for Empty {
    fn fill_buf(&mut self) -> io::Result<Option<&[u8]>> {
        Ok(Some(&[]))
    }

    fn consume(&mut self, _amt: usize) {}
}
