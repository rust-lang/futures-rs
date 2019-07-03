//! Await
//!
//! This module contains a number of functions and combinators for working
//! with `async`/`await` code.

use futures_core::future::Future;
use futures_core::stream::Stream;

#[doc(hidden)]
pub use futures_core::future::FusedFuture;
#[doc(hidden)]
pub use futures_core::stream::FusedStream;

#[macro_use]
mod poll;
pub use self::poll::*;

#[macro_use]
mod pending;
pub use self::pending::*;

// Primary export is a macro
#[macro_use]
mod join;

// Primary export is a macro
#[cfg(feature = "select-macro")]
mod select_mod;
#[cfg(feature = "select-macro")]
pub use self::select_mod::*;

#[doc(hidden)]
#[inline(always)]
pub fn assert_unpin<T: Unpin>(_: &T) {}

#[doc(hidden)]
#[inline(always)]
pub fn assert_fused_future<T: Future + FusedFuture>(_: &T) {}

#[doc(hidden)]
#[inline(always)]
pub fn assert_fused_stream<T: Stream + FusedStream>(_: &T) {}
