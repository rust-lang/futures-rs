//! Task notification

cfg_target_has_atomic! {
    #[cfg(feature = "alloc")]
    mod arc_wake;
    #[cfg(feature = "alloc")]
    pub use self::arc_wake::ArcWake;

    #[cfg(feature = "alloc")]
    mod waker;
    #[cfg(feature = "alloc")]
    pub use self::waker::waker;

    #[cfg(feature = "alloc")]
    mod waker_ref;
    #[cfg(feature = "alloc")]
    pub use self::waker_ref::{waker_ref, WakerRef};

    pub use futures_core::task::__internal::AtomicWaker;
}

mod noop_waker;
pub use self::noop_waker::noop_waker;
#[cfg(feature = "std")]
pub use self::noop_waker::noop_waker_ref;

mod spawn;
pub use self::spawn::{LocalSpawnExt, SpawnExt};

pub use futures_core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
