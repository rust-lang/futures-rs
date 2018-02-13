//! Core traits and types for asynchronous operations in Rust.

#![no_std]
//#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures-core/0.2")]

#[macro_use]
#[cfg(feature = "std")]
extern crate std;

extern crate anchor_experiment;

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
pub use future::{Future, FutureMove, IntoFuture};

pub mod stream;
pub use stream::Stream;

mod task_impl;
pub mod task;

pub mod never;
