//! Tools for working with tasks.

#![no_std]
#![doc(test(no_crate_inject, attr(allow(dead_code, unused_assignments, unused_variables))))]
#![warn(
    missing_docs,
    unsafe_op_in_unsafe_fn,
    clippy::alloc_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core
)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod spawn;
pub use crate::spawn::{LocalSpawn, Spawn, SpawnError};

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
mod arc_wake;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use crate::arc_wake::ArcWake;

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
mod waker;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use crate::waker::waker;

#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
mod waker_ref;
#[cfg(target_has_atomic = "ptr")]
#[cfg(feature = "alloc")]
pub use crate::waker_ref::{waker_ref, WakerRef};

mod future_obj;
pub use crate::future_obj::{FutureObj, LocalFutureObj, UnsafeFutureObj};

mod noop_waker;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
#[doc(no_inline)]
pub use alloc::task::Wake;
#[doc(no_inline)]
pub use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub use crate::noop_waker::{noop_waker, noop_waker_ref};
