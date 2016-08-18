//! I/O fused with futures and streams
//!
//! Building on top of the `futures` crate, the purpose of this crate is to
//! provide the abstractions necessary for interoperating I/O streams in an
//! asynchronous fashion.
//!
//! ## Installation
//!
//! Currently it's recommended to use the git version of this repository as it's
//! in active development, but this will be published to crates.io in the near
//! future!
//!
//! ```toml
//! [dependencies]
//! futures = { git = "https://github.com/alexcrichton/futures-rs" }
//! futures-io = { git = "https://github.com/alexcrichton/futures-rs" }
//! ```
//!
//! ## Core abstraction
//!
//! At its core this crate doesn't actually provide any traits, but rather
//! dictates particular semantics when working with I/O to easily interoperate
//! with futures. Like the `Future::poll` method, the intention is that you
//! simply perform the I/O as you normally would, and if `WouldBlock` is
//! returned then the task will automatically try again once the data is
//! available.
//!
//! Otherwise this crate is intended to be generally interoperable with the
//! standard library's `Read` and `Write` trait. All of the combinators provided
//! here work with `Read` and `Write` generics, but they assume that if
//! `WouldBlock` is returned that the current task associated with the future
//! will get notified once the I/O is ready to proceed again.

#![deny(missing_docs)]

#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;

use std::io;

use futures::BoxFuture;
use futures::stream::BoxStream;

/// A convenience typedef around a `Future` whose error component is `io::Error`
pub type IoFuture<T> = BoxFuture<T, io::Error>;

/// A convenience typedef around a `Stream` whose error component is `io::Error`
pub type IoStream<T> = BoxStream<T, io::Error>;

/// A convenience macro for working with `io::Result<T>` from the `Read` and
/// `Write` traits.
///
/// This macro takes `io::Result<T>` as input, and returns `T` as the output. If
/// the input type is of the `Err` variant, then `Poll::NotReady` is returned if
/// it indicates `WouldBlock` or otherwise `Err` is returned.
#[macro_export]
macro_rules! try_nb {
    ($e:expr) => (match $e {
        Ok(t) => t,
        Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => {
            return ::futures::Poll::NotReady
        }
        Err(e) => return ::futures::Poll::Err(e.into()),
    })
}

mod copy;
mod flush;
mod read_exact;
mod read_to_end;
mod task;
mod window;
mod write_all;
pub use copy::{copy, Copy};
pub use flush::{flush, Flush};
pub use read_exact::{read_exact, ReadExact};
pub use read_to_end::{read_to_end, ReadToEnd};
pub use task::{TaskIo, TaskIoRead, TaskIoWrite};
pub use window::Window;
pub use write_all::{write_all, WriteAll};
