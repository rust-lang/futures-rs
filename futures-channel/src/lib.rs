//! Asynchronous channels.
//!
//! This crate provides channels that can be used to communicate between
//! asynchronous tasks.

#![feature(futures_api)]
#![cfg_attr(feature = "cfg-target-has-atomic", feature(cfg_target_has_atomic))]
#![cfg_attr(all(feature = "alloc", not(feature = "std")), feature(alloc))]

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.13/futures_channel")]

#[cfg(all(feature = "cfg-target-has-atomic", not(feature = "nightly")))]
compile_error!("The `cfg-target-has-atomic` feature requires the `nightly` feature as an explicit opt-in to unstable features");

#[cfg(all(feature = "alloc", not(any(feature = "std", feature = "nightly"))))]
compile_error!("The `alloc` feature without `std` requires the `nightly` feature active to explicitly opt-in to unstable features");

#[cfg(all(feature = "alloc", not(feature = "std")))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std as alloc;

macro_rules! cfg_target_has_atomic {
    ($($item:item)*) => {$(
        #[cfg_attr(
            feature = "cfg-target-has-atomic",
            cfg(all(target_has_atomic = "cas", target_has_atomic = "ptr"))
        )]
        $item
    )*};
}

cfg_target_has_atomic! {
    #[cfg(feature = "alloc")]
    mod lock;
    #[cfg(feature = "alloc")]
    pub mod mpsc;
    #[cfg(feature = "alloc")]
    pub mod oneshot;
}
