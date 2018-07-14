//! Task notification

mod context;
pub use self::context::ContextExt;

#[cfg_attr(
    feature = "nightly",
    cfg(all(target_has_atomic = "cas", target_has_atomic = "ptr"))
)]
mod atomic_waker;
#[cfg_attr(
    feature = "nightly",
    cfg(all(target_has_atomic = "cas", target_has_atomic = "ptr"))
)]
pub use self::atomic_waker::AtomicWaker;
