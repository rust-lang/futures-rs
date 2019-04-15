//! Task notification

cfg_target_has_atomic! {
    /// A macro for creating a `RawWaker` vtable for a type that implements
    /// the `ArcWake` trait.
    #[cfg(feature = "alloc")]
    macro_rules! waker_vtable {
        ($ty:ident) => {
            &RawWakerVTable::new(
                clone_arc_raw::<$ty>,
                wake_arc_raw::<$ty>,
                wake_by_ref_arc_raw::<$ty>,
                drop_arc_raw::<$ty>,
            )
        };
    }

    #[cfg(feature = "alloc")]
    mod arc_wake;
    #[cfg(feature = "alloc")]
    pub use self::arc_wake::ArcWake;

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
pub use self::spawn::{SpawnExt, LocalSpawnExt};

// re-export for `select!`
#[doc(hidden)]
pub use futures_core::task::{Context, Poll, Waker};
