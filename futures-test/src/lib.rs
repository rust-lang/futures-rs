//! Utilities to make testing [`Future`s](futures_core::future::Future) easier

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]
#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

#[cfg(not(feature = "std"))]
compile_error!(
    "`futures-test` must have the `std` feature activated, this is a default-active feature"
);

// Not public API.
#[doc(hidden)]
#[cfg(feature = "std")]
pub mod __private {
    pub use futures_core::{future, stream, task};
    pub use std::{
        option::Option::{None, Some},
        pin::Pin,
        result::Result::{Err, Ok},
    };

    pub mod assert {
        pub use crate::assert::*;
    }
}

#[macro_use]
#[cfg(feature = "std")]
mod assert;

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
