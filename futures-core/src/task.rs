//! Task notification.

pub use core::task::{
    Context, Poll, Spawn,
    Waker, LocalWaker, UnsafeWake,
    SpawnErrorKind, SpawnObjError, SpawnLocalObjError,
};

if_std! {
    pub use std::task::{Wake, local_waker, local_waker_from_nonlocal};
}
