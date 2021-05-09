#![cfg(nightly)]
#![no_std]
#![feature(cfg_target_has_atomic)]

#[cfg(feature = "futures-core-alloc")]
#[cfg(target_has_atomic = "ptr")]
pub use futures_core::task::__internal::AtomicWaker as _;

#[cfg(feature = "futures-task-alloc")]
#[cfg(target_has_atomic = "ptr")]
pub use futures_task::ArcWake as _;

#[cfg(feature = "futures-channel-alloc")]
#[cfg(target_has_atomic = "ptr")]
pub use futures_channel::oneshot as _;

#[cfg(any(feature = "futures", feature = "futures-alloc"))]
#[cfg(target_has_atomic = "ptr")]
pub use futures::task::AtomicWaker as _;

#[cfg(feature = "futures-alloc")]
#[cfg(target_has_atomic = "ptr")]
pub use futures::stream::FuturesOrdered as _;

#[cfg(any(feature = "futures-util", feature = "futures-util-alloc"))]
#[cfg(target_has_atomic = "ptr")]
pub use futures_util::task::AtomicWaker as _;

#[cfg(feature = "futures-util-alloc")]
#[cfg(target_has_atomic = "ptr")]
pub use futures_util::stream::FuturesOrdered as _;
