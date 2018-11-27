//! Built-in executors and related tools.

#![feature(pin, arbitrary_self_types, futures_api)]

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.10/futures_executor")]

#[cfg(feature = "std")]
mod local_pool;
#[cfg(feature = "std")]
pub use crate::local_pool::{block_on, block_on_stream, BlockingStream, LocalPool, LocalSpawner};

#[cfg(feature = "std")]
mod unpark_mutex;
#[cfg(feature = "std")]
mod thread_pool;
#[cfg(feature = "std")]
pub use crate::thread_pool::{ThreadPool, ThreadPoolBuilder};

#[cfg(feature = "std")]
mod enter;
#[cfg(feature = "std")]
pub use crate::enter::{enter, Enter, EnterError};
