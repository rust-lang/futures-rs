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
    pub use futures_executor::block_on;
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

/// Enables an `async` test function. The generated future will be run to completion with
/// [`futures_executor::block_on`](futures_executor::block_on).
///
/// ```
/// #[futures_test::test]
/// async fn my_test() {
///     let fut = async { true };
///     assert!(fut.await);
/// }
/// ```
///
/// This is equivalent to the following code:
///
/// ```
/// #[test]
/// fn my_test() {
///     futures::executor::block_on(async move {
///         let fut = async { true };
///         assert!(fut.await);
///     })
/// }
/// ```
#[cfg(feature = "std")]
pub use futures_macro::test_internal as test;
