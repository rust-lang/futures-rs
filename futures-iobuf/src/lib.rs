//! I/O buffers for use with futures
//!
//! Buffer mangament tends to be quite critical for performance with lots of I/O
//! in flight, and when throwing futures into the mix you've got the added
//! requirement of `Send + 'static` for each future itself. Where previously
//! slices would otherwise be used instead this crate has an `IoBuf` abstraction
//! for an underlying reference-counted buffer to allow for easier management.
//!
//! This crate allows a buffer to be shared among a number of users, where each
//! user "owns" a particular portion of the buffer. More details on `IoBuf`
//! itself.

#![deny(missing_docs)]

extern crate futures;
extern crate futures_io;
#[macro_use]
extern crate log;

mod iobuf;
pub use self::iobuf::IoBuf;
