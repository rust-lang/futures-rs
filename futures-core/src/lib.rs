//! Core traits and types for asynchronous operations in Rust.

#![cfg_attr(feature = "nightly", feature(pin, arbitrary_self_types, specialization))]

#![no_std]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures-core/0.3.0")]

#![cfg_attr(feature = "nightly", feature(cfg_target_has_atomic))]
#![cfg_attr(feature = "nightly", feature(pin))]

#[macro_use]
#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "either")]
extern crate either;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

#[macro_export]
macro_rules! pinned_deref {
    ($e:expr) => (
        ::core::mem::Pin::new_unchecked(
            &mut **::core::mem::Pin::get_mut(&mut $e)
        )
    )
}

#[macro_export]
macro_rules! pinned_field {
    ($e:expr, $f:tt) => (
        ::core::mem::Pin::new_unchecked(
            &mut ::core::mem::Pin::get_mut(&mut $e).$f
        )
    )
}

mod poll;
pub use poll::Poll;

pub mod future;
pub use future::Future;

pub mod stream;
pub use stream::Stream;

pub mod task;

pub mod executor;

/// Standin for the currently-unstable `std::marker::Unpin` trait
#[cfg(not(feature = "nightly"))]
pub unsafe trait Unpin {}
#[cfg(not(feature = "nightly"))]
mod impls;

#[cfg(feature = "nightly")]
pub use core::marker::Unpin;
#[cfg(feature = "nightly")]
mod impls_nightly;
