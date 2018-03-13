use futures_core::{Async, Poll, task};
use futures_io::{AsyncRead, AsyncWrite};
use std::{fmt, io};
use std::string::String;
use std::vec::Vec;

/// A simple wrapper type which allows types which implement only
/// implement `std::io::Read` or `std::io::Write`
/// to be used in contexts which expect an `AsyncRead` or `AsyncWrite`.
///
/// If these types issue an error with the kind `io::ErrorKind::WouldBlock`,
/// it is expected that they will notify the current task on readiness.
/// Synchronous `std` types should not issue errors of this kind and
/// are safe to use in this context. However, using these types with
/// `AllowStdIo` will cause the event loop to block, so they should be used
/// with care.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AllowStdIo<T>(T);

macro_rules! try_with_interrupt {
    ($e:expr) => {
        loop {
            match $e {
                Ok(e) => {
                    break e;
                }
                Err(ref e) if e.kind() == ::std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }
}

impl<T> AllowStdIo<T> {
    /// Creates a new `AllowStdIo` from an existing IO object.
    pub fn new(io: T) -> Self {
        AllowStdIo(io)
    }

    /// Returns a reference to the contained IO object.
    pub fn get_ref(&self) -> &T {
        &self.0
    }

    /// Returns a mutable reference to the contained IO object.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }

    /// Consumes self and returns the contained IO object.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> io::Write for AllowStdIo<T> where T: io::Write {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }
    fn write_fmt(&mut self, fmt: fmt::Arguments) -> io::Result<()> {
        self.0.write_fmt(fmt)
    }
}

impl<T> AsyncWrite for AllowStdIo<T> where T: io::Write {
    fn poll_write(&mut self, _: &mut task::Context, buf: &[u8])
        -> Poll<usize, io::Error>
    {
        Ok(Async::Ready(try_with_interrupt!(io::Write::write(&mut self.0, buf))))
    }

    fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), io::Error> {
        Ok(Async::Ready(try_with_interrupt!(io::Write::flush(self))))
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), io::Error> {
        self.poll_flush(cx)
    }
}

impl<T> io::Read for AllowStdIo<T> where T: io::Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    // TODO: implement the `initializer` fn when it stabilizes.
    // See rust-lang/rust #42788
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.0.read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }
}

impl<T> AsyncRead for AllowStdIo<T> where T: io::Read {
    fn poll_read(&mut self, _: &mut task::Context, buf: &mut [u8])
        -> Poll<usize, io::Error>
    {
        Ok(Async::Ready(try_with_interrupt!(io::Read::read(&mut self.0, buf))))
    }
}
