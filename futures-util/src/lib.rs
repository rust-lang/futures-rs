//! Combinators and utilities for working with `Future`s, `Stream`s, `Sink`s,
//! and the `AsyncRead` and `AsyncWrite` traits.

#![feature(async_await, pin, arbitrary_self_types, futures_api, unsized_locals)]
#![cfg_attr(feature = "std", feature(await_macro))]
#![cfg_attr(feature = "nightly", feature(cfg_target_has_atomic))]

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.10/futures_util")]

#[macro_use]
mod macros;

#[cfg(feature = "std")]
#[macro_use]
pub mod async_await;

#[cfg(feature = "std")]
#[doc(hidden)]
pub mod rand_reexport { // used by select!
    pub use rand::*;
}

#[doc(hidden)]
pub mod core_reexport {
    pub use core::*;
}

macro_rules! delegate_sink {
    ($field:ident) => {
        fn poll_ready(
            mut self: Pin<&mut Self>,
            lw: &$crate::core_reexport::task::LocalWaker,
        ) -> $crate::core_reexport::task::Poll<Result<(), Self::SinkError>> {
            self.$field().poll_ready(lw)
        }

        fn start_send(
            mut self: Pin<&mut Self>,
            item: Self::SinkItem
        ) -> Result<(), Self::SinkError> {
            self.$field().start_send(item)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            lw: &$crate::core_reexport::task::LocalWaker
        ) -> $crate::core_reexport::task::Poll<Result<(), Self::SinkError>> {
            self.$field().poll_flush(lw)
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            lw: &$crate::core_reexport::task::LocalWaker
        ) -> $crate::core_reexport::task::Poll<Result<(), Self::SinkError>> {
            self.$field().poll_close(lw)
        }
    }
}

pub mod future;
#[doc(hidden)] pub use crate::future::FutureExt;

pub mod try_future;
#[doc(hidden)] pub use crate::try_future::TryFutureExt;

pub mod stream;
#[doc(hidden)] pub use crate::stream::StreamExt;

pub mod try_stream;
#[doc(hidden)] pub use crate::try_stream::TryStreamExt;

pub mod sink;
#[doc(hidden)] pub use crate::sink::SinkExt;

pub mod task;

#[cfg(feature = "compat")]
pub mod compat;

#[cfg(feature = "std")]
pub mod io;
#[cfg(feature = "std")]
#[doc(hidden)] pub use crate::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "std")]
pub mod lock;
