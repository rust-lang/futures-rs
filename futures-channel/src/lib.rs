//! Asynchronous channels.
//!
//! This crate provides channels that can be used to communicate between
//! asynchronous tasks.

#![feature(pin, arbitrary_self_types, futures_api)]

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.8/futures_channel")]

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
