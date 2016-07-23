#![allow(missing_docs)]

use std::io;
use std::ops::BitOr;

use {Task, Poll};
use stream::Stream;

#[macro_export]
macro_rules! try_nb {
    ($e:expr) => (match $e {
        Ok(e) => Some(e),
        Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => None,
        Err(e) => return Err(e),
    })
}

mod buf_reader;
mod buf_writer;
mod chain;
mod copy;
mod read_to_end;
mod take;
mod task;
pub use self::buf_reader::BufReader;
pub use self::buf_writer::BufWriter;
pub use self::chain::{chain, Chain};
pub use self::copy::{copy, Copy};
pub use self::read_to_end::{read_to_end, ReadToEnd};
pub use self::take::{take, Take};
pub use self::task::TaskIo;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Ready {
    Read,
    Write,
    ReadWrite,
}

pub trait ReadTask: Stream<Item=Ready, Error=io::Error> {
    fn read(&mut self, task: &mut Task, buf: &mut [u8]) -> io::Result<usize>;
    fn read_to_end(&mut self,
                   task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize>;
}

pub trait WriteTask: Stream<Item=Ready, Error=io::Error> {
    fn write(&mut self, task: &mut Task, buf: &[u8]) -> io::Result<usize>;
    fn flush(&mut self, task: &mut Task) -> io::Result<()>;
}

impl<R: ?Sized> ReadTask for R
    where R: io::Read + Stream<Item=Ready, Error=io::Error>,
{
    fn read(&mut self, _task: &mut Task, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(self, buf)
    }

    fn read_to_end(&mut self,
                   _task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize> {
        io::Read::read_to_end(self, buf)
    }
}

impl<W: ?Sized> WriteTask for W
    where W: io::Write + Stream<Item=Ready, Error=io::Error>,
{
    fn write(&mut self, _task: &mut Task, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(self, buf)
    }

    fn flush(&mut self, _task: &mut Task) -> io::Result<()> {
        io::Write::flush(self)
    }
}

impl Ready {
    pub fn is_read(&self) -> bool {
        match *self {
            Ready::Read | Ready::ReadWrite => true,
            Ready::Write => false,
        }
    }

    pub fn is_write(&self) -> bool {
        match *self {
            Ready::Write | Ready::ReadWrite => true,
            Ready::Read => false,
        }
    }
}

impl BitOr for Ready {
    type Output = Ready;

    fn bitor(self, other: Ready) -> Ready {
        match (self, other) {
            (Ready::ReadWrite, _) |
            (_, Ready::ReadWrite) |
            (Ready::Write, Ready::Read) |
            (Ready::Read, Ready::Write) => Ready::ReadWrite,

            (Ready::Read, Ready::Read) => Ready::Read,
            (Ready::Write, Ready::Write) => Ready::Write,
        }
    }
}

impl Stream for io::Empty {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(None)
    }

    fn schedule(&mut self, task: &mut Task) {
        drop(task);
    }
}

impl Stream for io::Repeat {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(Some(Ready::Read))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}

impl Stream for io::Sink {
    type Item = Ready;
    type Error = io::Error;

    fn poll(&mut self, _task: &mut Task) -> Poll<Option<Ready>, io::Error> {
        Poll::Ok(Some(Ready::Write))
    }

    fn schedule(&mut self, task: &mut Task) {
        task.notify()
    }
}
