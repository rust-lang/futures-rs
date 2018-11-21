//! Futures-powered synchronization primitives.

mod mutex;
pub use self::mutex::{Mutex, MutexAcquire, MutexGuard};

mod bilock;
#[cfg(any(test, feature = "bench"))]
pub use self::bilock::{BiLock, BiLockAcquire, BiLockGuard, ReuniteError};
#[cfg(not(any(test, feature = "bench")))]
pub(crate) use self::bilock::BiLock;
