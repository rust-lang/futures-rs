//! Task notification.

pub use core::task::{Poll, Waker, LocalWaker, UnsafeWake};
if_std! {
    pub use std::task::{Wake, local_waker, local_waker_from_nonlocal};
}
