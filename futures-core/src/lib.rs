//! Core traits and types for asynchronous operations in Rust.

#![feature(pin, arbitrary_self_types)]

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
pub use poll::{Poll, PollResult};

pub mod future;
pub use future::{Future, IntoFuture};

mod async_trait;
pub use async_trait::{Async, AsyncResult, Ready, ready};

pub mod stream;
pub use stream::Stream;

pub mod task;

pub mod executor;

pub mod never;
pub use never::Never;
