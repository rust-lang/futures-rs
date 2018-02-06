//! Core traits and types for asynchronous operations in Rust.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures/0.2")]

#[macro_use]
#[cfg(feature = "std")]
extern crate std;

#[doc(hidden)]
#[macro_export]
macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
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

mod task_impl;
pub mod task;

pub mod never;
