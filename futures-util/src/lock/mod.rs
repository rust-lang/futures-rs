//! Futures-powered synchronization primitives.

#[cfg(feature = "std")]
mod mutex;
#[cfg(feature = "std")]
pub use self::mutex::{Mutex, MutexLockFuture, MutexGuard};

mod bilock;
#[cfg(any(test, feature = "bench"))]
pub use self::bilock::{BiLock, BiLockAcquire, BiLockGuard, ReuniteError};
#[cfg(not(any(test, feature = "bench")))]
pub(crate) use self::bilock::BiLock;
