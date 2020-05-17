//! Built-in executors and related tools.
//!
//! All items of this library are only available when the `std` feature of this
//! library is activated, and it is activated by default.

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]

#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

#![doc(html_root_url = "https://docs.rs/futures-executor/0.3.5")]

#[cfg(feature = "std")]
mod local_pool;
#[cfg(feature = "std")]
pub use crate::local_pool::{block_on, block_on_stream, BlockingStream, LocalPool, LocalSpawner};

#[cfg(feature = "thread-pool")]
#[cfg(feature = "std")]
mod unpark_mutex;
#[cfg(feature = "thread-pool")]
#[cfg(feature = "std")]
mod thread_pool;
#[cfg(feature = "thread-pool")]
#[cfg(feature = "std")]
pub use crate::thread_pool::{ThreadPool, ThreadPoolBuilder};

#[cfg(feature = "std")]
mod enter;
#[cfg(feature = "std")]
pub use crate::enter::{enter, Enter, EnterError};
