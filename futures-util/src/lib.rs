//! Combinators and utilities for working with futures-rs `Futures`s, `Stream`s, and `Sink`s.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures/0.1")]

#[macro_use]
extern crate futures_core;
extern crate futures_executor;
extern crate futures_sink;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

use futures_core::{Async, Future, IntoFuture, Poll, Stream};
use futures_core::task;
use futures_sink::{AsyncSink, Sink, StartSend};

#[macro_use]
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod lock;

pub mod future;
pub use future::FutureExt;

pub mod stream;
pub use stream::StreamExt;

pub mod sink;
pub use sink::SinkExt;

pub mod prelude {
    //! Prelude with common traits from the `futures-util` crate.
    pub use {FutureExt, StreamExt, SinkExt};
}
