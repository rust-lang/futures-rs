//! Built-in executors and related tools.

#![feature(pin, arbitrary_self_types, futures_api)]

#![no_std]

#![deny(missing_docs, missing_debug_implementations, warnings)]
#![deny(bare_trait_objects)]

#![doc(html_root_url = "https://docs.rs/futures-executor/0.2.0")]

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

#[cfg(feature = "std")]
#[macro_use]
extern crate futures_core;

#[cfg(not(feature = "std"))]
extern crate futures_core;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    #[macro_use]
    extern crate lazy_static;

    extern crate futures_util;
    extern crate futures_channel;
    extern crate num_cpus;

    mod local_pool;
    pub use local_pool::{block_on, block_on_stream, BlockingStream, LocalPool, LocalExecutor};

    mod unpark_mutex;
    mod thread_pool;
    pub use thread_pool::{ThreadPool, ThreadPoolBuilder};

    mod enter;
    pub use enter::{enter, Enter, EnterError};

    mod spawn;
    pub use spawn::{spawn, Spawn, spawn_with_handle, SpawnWithHandle, JoinHandle};
}
