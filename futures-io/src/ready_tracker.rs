use std::io::{self, Read, Write};

use futures::{Poll, Task};
use futures::stream::Stream;

use Ready;

/// An I/O object which can be used to track the read/write readiness of an
/// underlying object.
///
/// Many I/O objects implement a stream of readiness with "edge" semantics where
/// once a notification is returned another one will never be returned until the
/// stream is not ready for I/O. That is, once a stream receives a `Read`
/// notification, it will never receive another one until all the pending data
/// has been read.
///
/// This object will keep track of this and implements two methods,
/// `maybe_read_ready` and `maybe_write_ready`. If an appropriate notification
/// has been seen in the `Stream` implementation and `WouldBlock` has *not*
/// been seen yet from the `Read` and `Write` implementations, these methods
/// will return `true`. Otherwise, if `WouldBlock` has been seen, then the
/// methods will return `false`.
///
/// This abstraction can be useful when readiness notifications need to be
/// translated to different readiness notifications at a particular I/O layer.
/// For example if an underlying stream is readable it doesn't necessarily mean
/// the outer stream may always be readable, and this object can be used to keep
/// track of all the notifications.
pub struct ReadyTracker<S> {
    inner: S,
    read_ready: bool,
    write_ready: bool,
}

impl<S> ReadyTracker<S>
    where S: Stream<Item=Ready, Error=io::Error>,
{
    /// Creates a new I/O object ready to track read/write notifications.
    pub fn new(s: S) -> ReadyTracker<S> {
        ReadyTracker {
            inner: s,
            read_ready: true,
            write_ready: true,
        }
    }
}

impl<S> ReadyTracker<S> {
    /// Returns whether the underlying stream might be ready for a read.
    ///
    /// A stream may be ready for a read when a `Read` notification was seen
    /// from the `Stream::poll` implementation, and the object has not be `read`
    /// enough to see `WouldBlock` yet.
    pub fn maybe_read_ready(&self) -> bool {
        self.read_ready
    }

    /// Returns whether the underlying stream might be ready for a write.
    ///
    /// A stream may be ready for a write when a `Write` notification was seen
    /// from the `Stream::poll` implementation, and the object has not be
    /// `write` enough to see `WouldBlock` yet.
    pub fn maybe_write_ready(&self) -> bool {
        self.write_ready
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

