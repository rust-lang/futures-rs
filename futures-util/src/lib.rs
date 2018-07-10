//! Combinators and utilities for working with `Future`s, `Stream`s, `Sink`s,
//! and the `AsyncRead` and `AsyncWrite` traits.

#![feature(async_await, await_macro, pin, arbitrary_self_types, futures_api)]

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://docs.rs/futures-util/0.3.0-alpha")]

#[macro_use]
mod macros;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

#[doc(hidden)]
pub mod core_reexport {
    pub use core::{mem, marker, future, task};
}

macro_rules! delegate_sink {
    ($field:ident) => {
        fn poll_ready(
            mut self: PinMut<Self>,
            cx: &mut $crate::core_reexport::task::Context,
        ) -> $crate::core_reexport::task::Poll<Result<(), Self::SinkError>> {
            self.$field().poll_ready(cx)
        }

        fn start_send(
            mut self: PinMut<Self>,
            item: Self::SinkItem
        ) -> Result<(), Self::SinkError> {
            self.$field().start_send(item)
        }

        fn poll_flush(
            mut self: PinMut<Self>,
            cx: &mut $crate::core_reexport::task::Context
        ) -> $crate::core_reexport::task::Poll<Result<(), Self::SinkError>> {
            self.$field().poll_flush(cx)
        }

        fn poll_close(
            mut self: PinMut<Self>,
            cx: &mut $crate::core_reexport::task::Context
        ) -> $crate::core_reexport::task::Poll<Result<(), Self::SinkError>> {
            self.$field().poll_close(cx)
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

if_std! {
    // FIXME: currently async/await is only available with std
    pub mod async_await;

    pub mod io;
    #[doc(hidden)] pub use crate::io::{AsyncReadExt, AsyncWriteExt};

    #[cfg(any(test, feature = "bench"))]
    pub mod lock;
    #[cfg(not(any(test, feature = "bench")))]
    mod lock;
}
