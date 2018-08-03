//! Task notification

mod executor;
pub use self::executor::{ExecutorExt, SpawnError};

if_std! {
    pub use self::executor::JoinHandle;

    mod local_waker_ref;
    pub use self::local_waker_ref::{local_waker_ref, local_waker_ref_from_nonlocal, LocalWakerRef};
}

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
