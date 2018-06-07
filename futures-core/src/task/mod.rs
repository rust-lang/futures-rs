//! Task notification.

use core::mem::{self, PinMut};

use {Future, Poll};

pub use core::task::{UnsafeWake, Waker};
#[cfg(feature = "std")]
pub use std::task::Wake;

pub use core::task::Context;

#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
mod atomic_waker;
#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
pub use self::atomic_waker::AtomicWaker;

pub use core::task::{TaskObj, UnsafePoll};

if_std! {
    use std::boxed::Box;

    pub trait TaskObjExt {
        /// Create a new `TaskObj` by boxing the given future.
        fn new<F: Future<Output = ()> + Send + 'static>(f: F) -> TaskObj;
    }

    impl TaskObjExt for TaskObj {
        /// Create a new `TaskObj` by boxing the given future.
        fn new<F: Future<Output = ()> + Send + 'static>(f: F) -> TaskObj {
            TaskObj::from_poll_task(Box::new(f))
        }
    }

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
                .spawn_obj(TaskObj::new(f)).unwrap()
        }
    }
}
