//! Combinators and utilities for working with futures-rs `Futures`s, `Stream`s, and `Sink`s.

#![no_std]
//#![deny(missing_docs, missing_debug_implementations, warnings)]
#![doc(html_root_url = "https://docs.rs/futures/0.1")]

#[macro_use]
extern crate futures_core;
extern crate futures_sink;
extern crate anchor_experiment;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

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
