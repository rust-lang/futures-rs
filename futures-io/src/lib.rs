#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;

use std::io;
use std::ops::BitOr;

use futures::Task;
use futures::stream::Stream;

#[macro_export]
macro_rules! try_nb {
    ($e:expr) => (match $e {
        Ok(e) => Some(e),
        Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => None,
        Err(e) => return Err(e),
    })
}

mod impls;

mod buf_reader;
mod buf_writer;
mod chain;
mod copy;
mod empty;
mod flush;
mod read_to_end;
mod repeat;
mod ready_tracker;
mod sink;
mod take;
mod task;
pub use self::buf_reader::BufReader;
pub use self::buf_writer::BufWriter;
pub use self::chain::{chain, Chain};
pub use self::copy::{copy, Copy};
pub use self::empty::{empty, Empty};
pub use self::flush::{flush, Flush};
pub use self::read_to_end::{read_to_end, ReadToEnd};
pub use self::ready_tracker::ReadyTracker;
pub use self::repeat::{repeat, Repeat};
pub use self::sink::{sink, Sink};
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
