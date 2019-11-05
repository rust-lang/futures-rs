//! Utilities to make testing [`Future`s](futures_core::future::Future) easier

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]

#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

#![doc(html_root_url = "https://docs.rs/futures-test/0.3.0")]

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
pub mod io;

mod interleave_pending;
