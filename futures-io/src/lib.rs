//! Future-based abstractions for dealing with I/O
//!
//! Building on top of the `futures` crate the purpose of this crate is to
//! provide the abstractions necessary for interoperating I/O streams in an
//! asynchronous fashion.
//!
//! At its core is the abstraction of I/O objects as a collection of `Read`,
//! `Write`, and `Stream<Item=Ready, Error=io::Error>` implementations. This can
//! then be used to define a number of combinators and then later define further
//! abstractions on these streams.
//!
//! Note that this library is currently a work in progress, but stay tuned for
//! updates!

#![deny(missing_docs)]

#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;

use std::io;
use std::ops::BitOr;

use futures::{Task, Future};
use futures::stream::Stream;

/// A macro to assist with dealing with `io::Result<T>` types where the error
/// may have the type `WouldBlock`.
///
/// Converts all `Ok` values to `Some`, `WouldBlock` errors to `None`, and
/// otherwise returns all other errors upwards the stack.
#[macro_export]
macro_rules! try_nb {
    ($e:expr) => (match $e {
        Ok(e) => Some(e),
        Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => None,
        Err(e) => return Err(::std::convert::From::from(e)),
    })
}

/// A convenience typedef around a `Future` whose error component is `io::Error`
pub type IoFuture<T> = Future<Item=T, Error=io::Error>;

/// A convenience typedef around a `Stream` whose error component is `io::Error`
pub type IoStream<T> = Stream<Item=T, Error=io::Error>;

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
pub use self::task::{TaskIo, TaskIoRead, TaskIoWrite};

/// Readiness notifications that a stream can deliver.
///
/// This is the primary point element yielded in `Stream` implementations on I/O
/// objects, indicating whether it is ready to be read or written to.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Ready {
    /// The I/O object is ready for a read.
    Read,
    /// The I/O object is ready for a write.
    Write,
    /// The I/O object is ready for both reading and writing.
    ReadWrite,
}

/// A trait representing streams that can be read within the context of a
/// future's `Task`.
///
/// This is a trait used to implement some of the "terminal" abstractions
/// provided by this crate. It is less general than the `Read` trait, and all
/// types which implement `Read` also implement this trait.
///
/// The main purpose of this trait is to allow insertion of I/O objects into
/// task-local storage but still allow for a `Read` implementation on the
/// returned handle.
pub trait ReadTask: Stream<Item=Ready, Error=io::Error> {
    /// Reads bytes into a buffer, optionally using `task` as a source of
    /// storage to draw from.
    ///
    /// Otherwise behaves the same as [`Read::read`][stdread].
    ///
    /// [stdread]: https://doc.rust-lang.org/std/io/trait.Read.html#tymethod.read
    fn read(&mut self, task: &mut Task, buf: &mut [u8]) -> io::Result<usize>;

    /// Reads as much information as possible from this underlying stream into
    /// the vector provided, optionally using the `task` as a source of storage
    /// to draw from.
    ///
    /// Otherwise behaves the same as [`Read::read_to_end`][stdreadtoend].
    ///
    /// [stdreadtoend]: https://doc.rust-lang.org/std/io/trait.Read.html#tymethod.read_to_end
    fn read_to_end(&mut self,
                   task: &mut Task,
                   buf: &mut Vec<u8>) -> io::Result<usize>;
}

/// A trait representing streams that can be written to within the context of a
/// future's `Task`.
///
/// This is a trait used to implement some of the "terminal" abstractions
/// provided by this crate. It is less general than the `Write` trait, and all
/// types which implement `Write` also implement this trait.
///
/// The main purpose of this trait is to allow insertion of I/O objects into
/// task-local storage but still allow for a `Write` implementation on the
/// returned handle.
pub trait WriteTask: Stream<Item=Ready, Error=io::Error> {
    /// Writes a list of bytes into this object, optionally using a `task` as a
    /// source of storage to draw from.
    ///
    /// Otherwise behaves the same as [`Write::write`][stdwrite]
    ///
    /// [stdwrite]: https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.write
    fn write(&mut self, task: &mut Task, buf: &[u8]) -> io::Result<usize>;

    /// Flushes any internal buffers of this object, optionally using a `task`
    /// as a source of storage to draw from.
    ///
    /// Otherwise behaves the same as [`Write::flush`][stdflush]
    ///
    /// [stdflush]: https://doc.rust-lang.org/std/io/trait.Write.html#tymethod.flush
    fn flush(&mut self, task: &mut Task) -> io::Result<()>;
}

impl Ready {
    /// Returns whether this readiness notification indicates that an object is
    /// readable.
    pub fn is_read(&self) -> bool {
        match *self {
            Ready::Read | Ready::ReadWrite => true,
            Ready::Write => false,
        }
    }

    /// Returns whether this readiness notification indicates that an object is
    /// writable.
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
