//! Task notification.

pub use core::task::{
    Context, Executor, Poll,
    Waker, LocalWaker, UnsafeWake,
    SpawnErrorKind, SpawnObjError, SpawnLocalObjError,
};
pub use core::future::FutureObj;

#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
mod atomic_waker;
#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
pub use self::atomic_waker::AtomicWaker;

if_std! {
    use crate::future::Future;
    use std::boxed::PinBox;

    pub use std::task::{Wake, local_waker, local_waker_from_nonlocal};

    /// Extension trait for `Context`, adding methods that require allocation.
    pub trait ContextExt {
        /// Spawn a future onto the default executor.
        ///
        /// # Panics
        ///
        /// This method will panic if the default executor is unable to spawn.
        ///
        /// To handle executor errors, use `Context::executor()` on instead.
        fn spawn<F>(&mut self, f: F) where F: Future<Output = ()> + 'static + Send;
    }

    impl<'a> ContextExt for Context<'a> {
        fn spawn<F>(&mut self, f: F) where F: Future<Output = ()> + 'static + Send {
            self.executor()
                .spawn_obj(FutureObj::new(PinBox::new(f))).unwrap()
        }
    }
}
