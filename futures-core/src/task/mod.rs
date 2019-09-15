//! Task notification.

#[macro_use]
mod poll;

mod spawn;
pub use self::spawn::{LocalSpawn, Spawn, SpawnError};

#[doc(hidden)]
pub mod __internal;

pub use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
