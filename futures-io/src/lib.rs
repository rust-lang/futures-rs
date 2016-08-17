//! I/O fused with futures and streams
//!
//! Building on top of the `futures` crate, the purpose of this crate is to
//! provide the abstractions necessary for interoperating I/O streams in an
//! asynchronous fashion.
//!
//! At its core is the abstraction of I/O objects as a collection of `Read`,
//! `Write`, and `Stream<Item=Ready, Error=io::Error>` implementations. This can
//! then be used to define a number of combinators and then later define further
//! abstractions on these streams.
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
//! ## Readiness
//!
//! This crate primarily provides adaptors, and traits useful for working with
//! objects that implement `Stream<Item=Ready, Error=io::Error>`. It's intended
//! that I/O objects like TCP streams, UDP sockets, TCP listeners, etc, can all
//! implement this interface and then get composed with one another.
//!
//! Many primitives provided in this crate are similar to the ones found in
//! `std::io`, with the added implementation of the `Stream` trait for
//! readiness.

#![deny(missing_docs)]

#[macro_use]
extern crate futures;
#[macro_use]
extern crate log;

use std::io;

use futures::BoxFuture;
use futures::stream::BoxStream;

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
pub type IoFuture<T> = BoxFuture<T, io::Error>;

/// A convenience typedef around a `Stream` whose error component is `io::Error`
pub type IoStream<T> = BoxStream<T, io::Error>;

mod copy;
mod flush;
mod read_exact;
mod read_to_end;
mod window;
mod write_all;
pub use copy::{copy, Copy};
pub use flush::{flush, Flush};
pub use read_exact::{read_exact, ReadExact};
pub use read_to_end::{read_to_end, ReadToEnd};
pub use window::Window;
pub use write_all::{write_all, WriteAll};
