//! Utilities to make testing [`Future`s](futures_core::future::Future) easier

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]

// mem::take requires Rust 1.40, matches! requires Rust 1.42
// Can be removed if the minimum supported version increased or if https://github.com/rust-lang/rust-clippy/issues/3941
// get's implemented.
#![allow(clippy::mem_replace_with_default, clippy::match_like_matches_macro)]

#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

#![doc(html_root_url = "https://docs.rs/futures-test/0.3.6")]

#[cfg(not(feature = "std"))]
compile_error!("`futures-test` must have the `std` feature activated, this is a default-active feature");

#[doc(hidden)]
#[cfg(feature = "std")]
pub use std as std_reexport;

#[doc(hidden)]
#[cfg(feature = "std")]
pub extern crate futures_core as futures_core_reexport;

#[macro_use]
#[doc(hidden)]
#[cfg(feature = "std")]
pub mod assert;

#[cfg(feature = "std")]
pub mod task;

#[cfg(feature = "std")]
pub mod future;

#[cfg(feature = "std")]
pub mod stream;

#[cfg(feature = "std")]
pub mod sink;

#[cfg(feature = "std")]
pub mod io;

mod assert_unmoved;
mod interleave_pending;
mod track_closed;
