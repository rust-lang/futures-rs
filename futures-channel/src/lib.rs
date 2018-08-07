//! Asynchronous channels.
//!
//! This crate provides channels that can be used to communicate between
//! asynchronous tasks.

#![feature(pin, arbitrary_self_types, futures_api)]

#![no_std]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-doc/0.3.0-alpha.2/futures_channel")]

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
