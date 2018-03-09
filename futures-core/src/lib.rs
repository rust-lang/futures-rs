//! Core traits and types for asynchronous operations in Rust.

#![no_std]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures-core/0.2.0-alpha")]

#[macro_use]
#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "either")]
extern crate either;

#[cfg(feature = "nightly")]
extern crate pin_api;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

macro_rules! if_not_std {
    ($($i:item)*) => ($(
        #[cfg(not(feature = "std"))]
        $i
    )*)
}

#[macro_use]
mod poll;
pub use poll::{Async, Poll};

pub mod future;
pub use future::{Future, IntoFuture};

pub mod stream;
pub use stream::Stream;

pub mod task;

pub mod executor;

pub mod never;
pub use never::Never;
