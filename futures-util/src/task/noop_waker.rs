//! Utilities for creating zero-cost wakers that don't do anything.
use futures_core::task::{LocalWaker, UnsafeWake, Waker};
use core::ptr::NonNull;

#[derive(Debug)]
struct NoopWake {
    _reserved: (),
}

unsafe impl UnsafeWake for NoopWake {
    unsafe fn clone_raw(&self) -> Waker {
        noop_waker()
    }

    unsafe fn drop_raw(&self) {}

    unsafe fn wake(&self) {}
}

fn noop_unsafe_wake() -> NonNull<dyn UnsafeWake> {
    static mut INSTANCE: NoopWake = NoopWake { _reserved: () };
    unsafe { NonNull::new_unchecked(&mut INSTANCE as *mut dyn UnsafeWake) }
}

fn noop_waker() -> Waker {
    unsafe { Waker::new(noop_unsafe_wake()) }
}

/// Create a new [`LocalWaker`](futures_core::task::LocalWaker) referencing a
/// singleton instance of [`NoopWake`].
#[inline]
pub fn noop_local_waker() -> LocalWaker {
    unsafe { LocalWaker::new(noop_unsafe_wake()) }
}

/// Get a thread local reference to a
/// [`LocalWaker`](futures_core::task::LocalWaker) referencing a singleton
/// instance of [`NoopWake`].
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures::task::noop_local_waker_ref;
/// let lw = noop_local_waker_ref();
/// lw.wake();
/// ```
#[inline]
pub fn noop_local_waker_ref() -> &'static LocalWaker {
    static NOOP_WAKE_REF: &(dyn UnsafeWake + Sync) = &NoopWake { _reserved: () };
    // Unsafety: `Waker` and `LocalWaker` are `repr(transparent)` wrappers around
    // `NonNull<dyn UnsafeWake>`, which has the same repr as `&(dyn UnsafeWake + Sync)`
    // So an &'static &(dyn UnsafeWake + Sync) can be unsafely cast to a
    // &'static LocalWaker
    #[allow(clippy::transmute_ptr_to_ptr)]
    unsafe {
        core::mem::transmute::<
            &&(dyn UnsafeWake + Sync),
            &'static LocalWaker,
        >(&NOOP_WAKE_REF)
    }
}
