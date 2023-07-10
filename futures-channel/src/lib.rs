//! Asynchronous channels.
//!
//! Like threads, concurrent tasks sometimes need to communicate with each
//! other. This module contains two basic abstractions for doing so:
//!
//! - [oneshot], a way of sending a single value from one task to another.
//! - [mpsc], a multi-producer, single-consumer channel for sending values
//!   between tasks, analogous to the similarly-named structure in the standard
//!   library.
//!
//! All items are only available when the `std` or `alloc` feature of this
//! library is activated, and it is activated by default.

#![cfg_attr(not(feature = "std"), no_std)]
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
#![allow(clippy::arc_with_non_send_sync)] // false positive https://github.com/rust-lang/rust-clippy/issues/11076

#[cfg(not(futures_no_atomic_cas))]
#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(not(futures_no_atomic_cas))]
#[cfg(feature = "alloc")]
mod lock;
#[cfg(not(futures_no_atomic_cas))]
#[cfg(feature = "std")]
pub mod mpsc;
#[cfg(not(futures_no_atomic_cas))]
#[cfg(feature = "alloc")]
pub mod oneshot;
