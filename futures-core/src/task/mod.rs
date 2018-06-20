//! Task notification.

use Future;

pub use core::task::{UnsafeWake, Waker, LocalWaker};

#[cfg(feature = "std")]
pub use std::task::{Wake, local_waker, local_waker_from_nonlocal};

pub use core::task::Context;

#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
mod atomic_waker;
#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
pub use self::atomic_waker::AtomicWaker;

pub use core::task::{TaskObj, UnsafeTask};

if_std! {
    use std::boxed::PinBox;

    /// Extension trait for `Context`, adding methods that require allocation.
    pub trait ContextExt {
        /// Spawn a future onto the default executor.
        ///
        /// # Panics
        ///
        /// This method will panic if the default executor is unable to spawn.
        ///
        /// To handle executor errors, use [executor()](self::Context::executor)
        /// instead.
        fn spawn<F>(&mut self, f: F) where F: Future<Output = ()> + 'static + Send;
    }

    impl<'a> ContextExt for Context<'a> {
        fn spawn<F>(&mut self, f: F) where F: Future<Output = ()> + 'static + Send {
            self.executor()
                .spawn_obj(TaskObj::new(PinBox::new(f))).unwrap()
        }
    }
}
