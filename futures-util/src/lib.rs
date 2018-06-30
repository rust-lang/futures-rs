//! Combinators and utilities for working with `Future`s, `Stream`s, `Sink`s,
//! and the `AsyncRead` and `AsyncWrite` traits.

#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://docs.rs/futures/0.1")]

#[cfg(test)]
extern crate futures_channel;
#[macro_use]
extern crate futures_core;
#[cfg(test)]
extern crate futures_executor;

extern crate futures_io;
extern crate futures_sink;

extern crate either;

#[cfg(feature = "std")]
use futures_core::{Future, Poll, task};

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

#[cfg(feature = "std")]
#[macro_use]
extern crate core;

#[doc(hidden)]
pub use futures_core::core_reexport;

macro_rules! delegate_sink {
    ($field:ident) => {
        fn poll_ready(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            self.$field().poll_ready(cx)
        }

        fn start_send(mut self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            self.$field().start_send(item)
        }

        fn poll_flush(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            self.$field().poll_flush(cx)
        }

        fn poll_close(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
            self.$field().poll_close(cx)
        }

    }
}

// FIXME: currently async/await is only available with std
#[cfg(feature = "std")]
pub mod await;

#[cfg(all(feature = "std", any(test, feature = "bench")))]
pub mod lock;
#[cfg(all(feature = "std", not(any(test, feature = "bench"))))]
mod lock;

pub mod future;
pub use future::FutureExt;

pub mod try_future;
pub use try_future::TryFutureExt;

#[cfg(feature = "std")]
pub mod io;
#[cfg(feature = "std")]
pub use io::{AsyncReadExt, AsyncWriteExt};

pub mod stream;
pub use stream::StreamExt;

pub mod sink;
pub use sink::SinkExt;
