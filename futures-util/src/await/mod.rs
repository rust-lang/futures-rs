//! Await
//!
//! This module contains a number of functions and combinators for working
//! with `async`/`await` code.

use futures_core::Future;
use core::marker::Unpin;

#[macro_use]
mod poll;
pub use self::poll::*;

#[macro_use]
mod pending;
pub use self::pending::*;

// Primary export is a macro
mod join;

// Primary export is a macro
mod select;

#[doc(hidden)]
pub fn assert_unpin<T: Future + Unpin>(_: &T) {}
