//! Combinators and utilities for working with `Future`s, `Stream`s, `Sink`s,
//! and the `AsyncRead` and `AsyncWrite` traits.

#![no_std]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures/0.1")]

#[macro_use]
extern crate futures_core;
extern crate futures_io;
extern crate futures_sink;
extern crate either;

#[cfg(feature = "std")]
use futures_core::{Async, Future, Poll, task};

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

macro_rules! delegate_sink {
    ($field:ident) => {
        fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
            self.$field.poll_ready(cx)
        }

        fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
            self.$field.start_send(item)
        }

        fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
            self.$field.poll_flush(cx)
        }

        fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
            self.$field.poll_close(cx)
        }

    }
}

#[cfg(feature = "std")]
pub mod lock;

pub mod future;
pub use future::FutureExt;

#[cfg(feature = "std")]
pub mod io;
#[cfg(feature = "std")]
pub use io::{AsyncReadExt, AsyncWriteExt};

pub mod stream;
pub use stream::StreamExt;

pub mod sink;
pub use sink::SinkExt;

pub mod prelude {
    //! Prelude containing the extension traits, which add functionality to
    //! existing asynchronous types.
    pub use {FutureExt, StreamExt, SinkExt};
    #[cfg(feature = "std")]
    pub use {AsyncReadExt, AsyncWriteExt};
}
