//! Core traits and types for asynchronous operations in Rust.

#![feature(arbitrary_self_types, futures_api)]
#![cfg_attr(feature = "nightly", feature(cfg_target_has_atomic))]

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.10/futures_core")]

pub mod future;
#[doc(hidden)] pub use self::future::{Future, FusedFuture, TryFuture};

pub mod stream;
#[doc(hidden)] pub use self::stream::{Stream, FusedStream, TryStream};

pub mod task;
#[doc(hidden)] pub use self::task::Poll;
