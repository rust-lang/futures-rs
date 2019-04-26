//! Asynchronous channels.
//!
//! This crate provides channels that can be used to communicate between
//! asynchronous tasks.

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.15/futures_channel")]

#[cfg(feature = "std")]
mod lock;
#[cfg(feature = "std")]
pub mod mpsc;
#[cfg(feature = "std")]
pub mod oneshot;
