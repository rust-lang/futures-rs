//! Utilities to make testing [`Future`s](futures_core::Future) easier

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(
    html_root_url = "https://rust-lang-nursery.github.io/futures-doc/0.3.0-alpha.5/futures_test"
)]

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
