//! Task notification.

use core::mem::{self, Pin};
use core::fmt;

use {Future, Poll};

mod wake;
pub use self::wake::{UnsafeWake, Waker};
#[cfg(feature = "std")]
pub use self::wake::Wake;

mod context;
pub use self::context::Context;

#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
mod atomic_waker;
#[cfg_attr(feature = "nightly", cfg(target_has_atomic = "ptr"))]
pub use self::atomic_waker::AtomicWaker;

/// A custom trait object for polling tasks, roughly akin to
/// `Box<Future<Output = ()> + Send>`.
pub struct TaskObj {
    ptr: *mut (),
    poll: unsafe fn(*mut (), &mut Context) -> Poll<()>,
    drop: unsafe fn(*mut ()),
}

impl fmt::Debug for TaskObj {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TakObj")
            .finish()
    }
}

unsafe impl Send for TaskObj {}
unsafe impl Sync for TaskObj {}

/// A custom implementation of a task trait object for `TaskObj`, providing
/// a hand-rolled vtable.
///
/// This custom representation is typically used only in `no_std` contexts,
/// where the default `Box`-based implementation is not available.
///
/// The implementor must guarantee that it is safe to call `poll` repeatedly (in
/// a non-concurrent fashion) with the result of `into_raw` until `drop` is
/// called.
pub unsafe trait UnsafePoll: Send + 'static {
    /// Convert a owned instance into a (conceptually owned) void pointer.
    fn into_raw(self) -> *mut ();

    /// Poll the task represented by the given void pointer.
    ///
    /// # Safety
    ///
    /// The trait implementor must guarantee that it is safe to repeatedly call
    /// `poll` with the result of `into_raw` until `drop` is called; such calls
    /// are not, however, allowed to race with each other or with calls to `drop`.
    unsafe fn poll(task: *mut (), cx: &mut Context) -> Poll<()>;

    /// Drops the task represented by the given void pointer.
    ///
    /// # Safety
    ///
    /// The trait implementor must guarantee that it is safe to call this
    /// function once per `into_raw` invocation; that call cannot race with
    /// other calls to `drop` or `poll`.
    unsafe fn drop(task: *mut ());
}

impl TaskObj {
    /// Create a `TaskObj` from a custom trait object representation.
    pub fn from_poll_task<T: UnsafePoll>(t: T) -> TaskObj {
        TaskObj {
            ptr: t.into_raw(),
            poll: T::poll,
            drop: T::drop,
        }
    }

    /// Poll the task.
    ///
    /// The semantics here are identical to that for futures, but unlike
    /// futures only an `&mut self` reference is needed here.
    pub fn poll_task(&mut self, cx: &mut Context) -> Poll<()> {
        unsafe {
            (self.poll)(self.ptr, cx)
        }
    }
}

impl Drop for TaskObj {
    fn drop(&mut self) {
        unsafe {
            (self.drop)(self.ptr)
        }
    }
}

if_std! {
    use std::boxed::Box;

    unsafe impl<F: Future<Output = ()> + Send + 'static> UnsafePoll for Box<F> {
        fn into_raw(self) -> *mut () {
            unsafe {
                mem::transmute(self)
            }
        }

        unsafe fn poll(task: *mut (), cx: &mut Context) -> Poll<()> {
            let ptr: *mut F = mem::transmute(task);
            let pin: Pin<F> = Pin::new_unchecked(&mut *ptr);
            pin.poll(cx)
        }

        unsafe fn drop(task: *mut ()) {
            let ptr: *mut F = mem::transmute(task);
            let boxed = Box::from_raw(ptr);
            drop(boxed)
        }
    }

    impl TaskObj {
        /// Create a new `TaskObj` by boxing the given future.
        pub fn new<F: Future<Output = ()> + Send + 'static>(f: F) -> TaskObj {
            TaskObj::from_poll_task(Box::new(f))
        }
    }
}
