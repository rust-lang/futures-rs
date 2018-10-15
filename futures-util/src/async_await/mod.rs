//! Await
//!
//! This module contains a number of functions and combinators for working
//! with `async`/`await` code.

use core::marker::Unpin;
use futures_core::future::Future;

#[doc(hidden)]
pub use futures_core::future::FusedFuture;

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
#[macro_use]
mod select;

#[doc(hidden)]
#[inline(always)]
pub fn assert_unpin<T: Future + Unpin>(_: &T) {}

#[doc(hidden)]
#[inline(always)]
pub fn assert_fused_future<T: Future + FusedFuture>(_: &T) {}
