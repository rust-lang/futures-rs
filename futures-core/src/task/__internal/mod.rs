#[cfg(has_atomic_cas)]
mod atomic_waker;
#[cfg(has_atomic_cas)]
pub use self::atomic_waker::AtomicWaker;
