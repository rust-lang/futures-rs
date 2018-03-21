//! Task notification.

use core::fmt;

mod wake;
pub use self::wake::{UnsafeWake, Waker};

mod context;
pub use self::context::Context;

if_std! {
    pub use self::wake::Wake;

    mod data;
    pub use self::data::LocalKey;
}

#[cfg(not(feature = "std"))]
mod data {
    pub struct LocalMap;

    pub fn local_map() -> LocalMap {
        LocalMap
    }
}


#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
mod atomic_waker;
#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
pub use self::atomic_waker::AtomicWaker;

/// A map storing task-local data.
pub struct LocalMap {
    #[allow(dead_code)]
    inner: data::LocalMap,
}

impl LocalMap {
    /// Create an empty set of task-local data.
    pub fn new() -> LocalMap {
        LocalMap { inner: data::local_map() }
    }
}

impl fmt::Debug for LocalMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LocalMap")
         .finish()
    }
}
