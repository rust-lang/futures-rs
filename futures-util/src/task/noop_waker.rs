//! Utilities for creating zero-cost wakers that don't do anything.
use futures_core::task::{LocalWaker, RawWaker, RawWakerVTable};
use core::ptr::null;
use core::cell::UnsafeCell;

unsafe fn noop_clone(_data: *const()) -> RawWaker {
    noop_raw_waker()
}

unsafe fn noop(_data: *const()) {
}

unsafe fn noop_into_waker(_data: *const()) -> Option<RawWaker> {
    Some(noop_raw_waker())
}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable {
    clone: noop_clone,
    drop_fn: noop,
    wake: noop,
    into_waker: noop_into_waker,
};

fn noop_raw_waker() -> RawWaker {
    RawWaker {
        data: null(),
        vtable: &NOOP_WAKER_VTABLE,
    }
}

/// Create a new [`LocalWaker`](futures_core::task::LocalWaker) which does
/// nothing when `wake()` is called on it. The [`LocalWaker`] can be converted
/// into a [`Waker`] which will behave the same way.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures::task::noop_local_waker;
/// let lw = noop_local_waker();
/// lw.wake();
/// ```
#[inline]
pub fn noop_local_waker() -> LocalWaker {
    unsafe {
        LocalWaker::new_unchecked(noop_raw_waker())
    }
}

/// Get a thread local reference to a
/// [`LocalWaker`](futures_core::task::LocalWaker) referencing a singleton
/// instance of a [`LocalWaker`] which panics when woken.
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
    thread_local! {
        static NOOP_WAKER_INSTANCE: UnsafeCell<LocalWaker> =
            UnsafeCell::new(noop_local_waker());
    }
    NOOP_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}

