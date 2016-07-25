use std::io::{self, Read, Write};

use {Poll, Task};
use stream::Stream;
use io::Ready;

pub struct ReadyTracker<S> {
    inner: S,
    read_ready: bool,
    write_ready: bool,
}

impl<S> ReadyTracker<S>
    where S: Stream<Item=Ready, Error=io::Error>,
{
    pub fn new(s: S) -> ReadyTracker<S> {
        ReadyTracker {
            inner: s,
            read_ready: false,
            write_ready: false,
        }
    }
}

impl<S> ReadyTracker<S> {
    pub fn maybe_read_ready(&self) -> bool {
        self.read_ready
    }

    pub fn maybe_write_ready(&self) -> bool {
        self.read_ready
    }
}

impl<S> Stream for ReadyTracker<S>
    where S: Stream<Item=Ready, Error=io::Error>,
{
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        match self.inner.poll(task) {
            Poll::Ok(Some(ready)) => {
                self.read_ready = self.read_ready || ready.is_read();
                self.write_ready = self.write_ready || ready.is_write();
                Poll::Ok(Some(ready))
            }
            other => other,
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        self.inner.schedule(task)
    }
}

fn is_wouldblock<T>(res: &io::Result<T>) -> bool {
    match *res {
        Ok(_) => false,
        Err(ref e) => e.kind() == io::ErrorKind::WouldBlock,
    }
}

impl<S: Read> Read for ReadyTracker<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let res = self.inner.read(buf);
        if is_wouldblock(&res) {
            debug!("read no longer ready");
            self.read_ready = false;
        }
        return res
    }
}

impl<S: Write> Write for ReadyTracker<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let res = self.inner.write(buf);
        if is_wouldblock(&res) {
            debug!("write no longer ready");
            self.write_ready = false;
        }
        return res
    }

    fn flush(&mut self) -> io::Result<()> {
        let res = self.inner.flush();
        if is_wouldblock(&res) {
            debug!("write no longer ready");
            self.write_ready = false;
        }
        return res
    }
}

