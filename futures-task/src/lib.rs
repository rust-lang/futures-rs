//! Tools for working with tasks.

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]

#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

#![doc(html_root_url = "https://docs.rs/futures-task-preview/0.3.0-alpha.19")]

#[cfg(feature = "alloc")]
extern crate alloc;

mod spawn;
pub use crate::spawn::{Spawn, SpawnError, LocalSpawn};

pub use futures_core::future::{FutureObj, LocalFutureObj};
