//! Core traits and types for asynchronous operations in Rust.

#![feature(pin, arbitrary_self_types, futures_api)]

#![no_std]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-doc/0.3.0-alpha.2/futures_core")]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "either")]
extern crate either;

#[doc(hidden)] pub use crate::future::Future;
#[doc(hidden)] pub use crate::future::TryFuture;

#[doc(hidden)] pub use crate::stream::Stream;
#[doc(hidden)] pub use crate::stream::TryStream;

#[doc(hidden)] pub use crate::task::Poll;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

pub mod future;

pub mod stream;

pub mod task;
