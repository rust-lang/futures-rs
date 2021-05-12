//! Utilities to make testing [`Future`s](futures_core::future::Future) easier

#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    single_use_lifetimes,
    unreachable_pub
)]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]

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
