//! Asynchronous channels.
//!
//! This crate provides channels that can be used to communicate between
//! asynchronous tasks.
//!
//! See [Asynchronous Programming in Rust](https://aturon.github.io/apr/) for an
//! overview.

#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures-channel/0.2")]
#![no_std]

#[cfg(feature = "std")]
extern crate std;

extern crate futures_core;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    mod lock;
    pub mod mpsc;
    pub mod oneshot;
}
