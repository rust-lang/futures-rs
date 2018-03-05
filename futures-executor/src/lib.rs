//! Built-in executors and related tools.

#![no_std]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/futures-executor/0.2.0-alpha")]

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    extern crate futures_core;
    extern crate futures_util;
    extern crate futures_channel;
    extern crate num_cpus;

    mod thread;

    mod local_pool;
    pub use local_pool::{block_on, LocalPool, LocalExecutor};

    mod unpark_mutex;
    mod thread_pool;
    pub use thread_pool::{ThreadPool, ThreadPoolBuilder};

    mod enter;
    pub use enter::{enter, Enter, EnterError};

    mod spawn;
    pub use spawn::{spawn, Spawn, spawn_with_handle, SpawnWithHandle};
}
