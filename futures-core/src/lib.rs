//! Core traits and types for asynchronous operations in Rust.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, /* unsafe_op_in_unsafe_fn */)] // unsafe_op_in_unsafe_fn requires Rust 1.52
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod future;
#[doc(no_inline)]
pub use self::future::{FusedFuture, Future, TryFuture};

pub mod stream;
#[doc(no_inline)]
pub use self::stream::{FusedStream, Stream, TryStream};

#[macro_use]
pub mod task;
