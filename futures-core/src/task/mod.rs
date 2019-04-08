//! Task notification.

mod spawn;
#[doc(hidden)]
pub mod __internal;
pub use self::spawn::{Spawn, LocalSpawn, SpawnError};

pub use core::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};
