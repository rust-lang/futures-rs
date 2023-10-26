#[cfg(any(not(futures_no_atomic_cas), feature = "portable-atomic"))]
mod atomic_waker;
#[cfg(any(not(futures_no_atomic_cas), feature = "portable-atomic"))]
pub use self::atomic_waker::AtomicWaker;
