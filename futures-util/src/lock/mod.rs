//! Futures-powered synchronization primitives.

mod bilock;
#[cfg(any(test, feature = "bench"))]
pub use self::bilock::{BiLock, BiLockAcquire, BiLockGuard, ReuniteError};
#[cfg(not(any(test, feature = "bench")))]
pub(crate) use self::bilock::BiLock;
