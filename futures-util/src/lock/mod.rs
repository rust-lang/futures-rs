//! Futures-powered synchronization primitives.

#[cfg(feature = "std")]
mod mutex;
#[cfg(feature = "std")]
pub use self::mutex::{Mutex, MutexLockFuture, MutexGuard};

#[cfg(any(feature = "sink", feature = "io"))]
#[allow(unreachable_pub)]
mod bilock;
#[cfg(any(feature = "sink", feature = "io"))]
#[cfg(any(test, feature = "bench"))]
pub use self::bilock::{BiLock, BiLockAcquire, BiLockGuard, ReuniteError};
#[cfg(any(feature = "sink", feature = "io"))]
#[cfg(not(any(test, feature = "bench")))]
pub(crate) use self::bilock::BiLock;
