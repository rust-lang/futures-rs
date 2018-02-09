//! A simple crate for executing work on a thread pool, and getting back a
//! future.
//!
//! This crate provides a simple thread pool abstraction for running work
//! externally from the current thread that's running. An instance of `Future`
//! is handed back to represent that the work may be done later, and further
//! computations can be chained along with it as well.
//!
//! ```rust
//! extern crate futures;
//! extern crate futures_cpupool;
//! extern crate futures_executor;
//!
//! use futures::prelude::*;
//! use futures_cpupool::CpuPool;
//! use futures_executor::current_thread::run;
//!
//! # fn long_running_future(a: u32) -> Box<futures::future::Future<Item = u32, Error = ()> + Send> {
//! #     Box::new(futures::future::result(Ok(a)))
//! # }
//! # fn main() {
//!
//! // Create a worker thread pool with four threads
//! let pool = CpuPool::new(4);
//!
//! // Execute some work on the thread pool, optionally closing over data.
//! let a = pool.spawn(long_running_future(2));
//! let b = pool.spawn(long_running_future(100));
//!
//! // Express some further computation once the work is completed on the thread
//! // pool.
//! let c = run(|cx| cx.block_on(a.join(b).map(|(a, b)| a + b))).unwrap();
//!
//! // Print out the result
//! println!("{:?}", c);
//! # }
//! ```

#![no_std]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![doc(html_root_url = "https://docs.rs/futures-cpupool/0.2")]

#[macro_use]
#[cfg(feature = "std")]
extern crate std;

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

if_std! {
    extern crate futures;
    extern crate num_cpus;

    mod unpark_mutex;
    mod pool;

    pub use self::pool::*;
}
