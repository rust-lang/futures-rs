//! Built-in executors and related tools.

#![feature(pin, arbitrary_self_types, futures_api)]

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.2/futures_executor")]

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    mod local_pool;
    pub use crate::local_pool::{block_on, block_on_stream, BlockingStream, LocalPool, LocalSpawn};

    mod unpark_mutex;
    mod thread_pool;
    pub use crate::thread_pool::{ThreadPool, ThreadPoolBuilder};

    mod enter;
    pub use crate::enter::{enter, Enter, EnterError};
}
