//! Utilities for creating zero-cost wakers that don't do anything.
use futures_core::task::{LocalWaker, UnsafeWake, Waker};
use std::cell::UnsafeCell;
use std::ptr::NonNull;

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
pub fn noop_local_waker_ref() -> &'static LocalWaker {
    thread_local! {
        static LOCAL_WAKER_INSTANCE: UnsafeCell<LocalWaker> =
            UnsafeCell::new(noop_local_waker());
    }
    LOCAL_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}
