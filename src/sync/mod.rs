//! Future-aware synchronization
//!
//! This module, which is modeled after `std::sync`, contains user-space
//! synchronization tools that work with futures, streams and sinks. In
//! particular, these synchronizers do *not* block physical OS threads, but
//! instead work at the task level.

pub mod oneshot;
pub mod spsc;
mod bilock;

pub use self::bilock::{BiLock, BiLockAcquire, BiLockAcquired};
