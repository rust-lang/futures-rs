//! Combinators and utilities for working with futures-rs `Futures`s, `Stream`s, and `Sink`s.

#![no_std]
#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures/0.1")]

#[macro_use]
extern crate futures_core;
extern crate futures_io;
extern crate futures_sink;

#[cfg(feature = "std")]
use futures_core::{Async, Future, Poll, Stream, task};
#[cfg(feature = "std")]
use futures_sink::Sink;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    extern crate bytes;
    #[macro_use]
    extern crate log;
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
pub use io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

pub mod stream;
pub use stream::StreamExt;

pub mod sink;
pub use sink::SinkExt;

pub mod prelude {
    //! Prelude with common traits from the `futures-util` crate.
    pub use {FutureExt, StreamExt, SinkExt};
    #[cfg(feature = "std")]
    pub use {AsyncReadExt, AsyncWriteExt};
}
