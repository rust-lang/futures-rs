//! Asynchronous synchronization primitives based on intrusive collections.
//!
//! This module provides various primitives for synchronizing concurrently
//! executing futures.

mod manual_reset_event;

pub use self::manual_reset_event::{
    LocalManualResetEvent, LocalWaitForEventFuture,
};

#[cfg(feature = "std")]
pub use self::manual_reset_event::{
    ManualResetEvent, WaitForEventFuture,
};

mod mutex;

pub use self::mutex::{
    LocalMutex, LocalMutexLockFuture, LocalMutexGuard,
};

#[cfg(feature = "std")]
pub use self::mutex::{
    Mutex, MutexLockFuture, MutexGuard,
};

