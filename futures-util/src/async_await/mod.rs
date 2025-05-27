//! Await
//!
//! This module contains a number of functions and combinators for working
//! with `async`/`await` code.

use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};

#[macro_use]
mod poll;
pub use self::poll::*;

#[macro_use]
mod pending;
pub use self::pending::*;

// Primary export is a macro
#[cfg(feature = "async-await-macro")]
mod join_mod;
#[cfg(feature = "async-await-macro")]
pub use self::join_mod::*;

// Primary export is a macro
#[cfg(feature = "async-await-macro")]
mod select_mod;
#[cfg(feature = "async-await-macro")]
pub use self::select_mod::*;

// Primary export is a macro
#[cfg(feature = "std")]
#[cfg(feature = "async-await-macro")]
mod stream_select_mod;
#[cfg(feature = "std")]
#[cfg(feature = "async-await-macro")]
pub use self::stream_select_mod::*;

#[doc(hidden)]
#[inline(always)]
pub fn assert_unpin<T: Unpin>(_: &T) {}

#[doc(hidden)]
#[inline(always)]
pub fn assert_fused_future<T: Future + FusedFuture>(_: &T) {}

#[doc(hidden)]
#[inline(always)]
pub fn assert_fused_stream<T: Stream + FusedStream>(_: &T) {}
