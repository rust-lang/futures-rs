//! Task notification.

#[macro_use]
mod poll;

mod spawn;
pub use self::spawn::{Spawn, LocalSpawn, SpawnError};

#[doc(hidden)]
pub mod __internal;

pub use core::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};
