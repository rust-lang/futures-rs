use futures_core::task::{LocalWaker, RawWaker, RawWakerVTable};
use core::cell::UnsafeCell;
use core::ptr::null;

unsafe fn noop_clone(_data: *const()) -> RawWaker {
    raw_panic_waker()
}

unsafe fn noop(_data: *const()) {
}

unsafe fn wake_panic(_data: *const()) {
    panic!("should not be woken");
}

unsafe fn noop_into_waker(_data: *const()) -> Option<RawWaker> {
    Some(raw_panic_waker())
}

const PANIC_WAKER_VTABLE: RawWakerVTable = RawWakerVTable {
    clone: noop_clone,
    drop_fn: noop,
    wake: wake_panic,
    into_waker: noop_into_waker,
};

fn raw_panic_waker() -> RawWaker {
    RawWaker {
        data: null(),
        vtable: &PANIC_WAKER_VTABLE,
    }
}

/// Create a new [`LocalWaker`](futures_core::task::LocalWaker) which will
/// panic when `wake()` is called on it. The [`LocalWaker`] can be converted
/// into a [`Waker`] which will behave the same way.
///
/// # Examples
///
/// ```should_panic
/// #![feature(futures_api)]
/// use futures_test::task::panic_local_waker;
///
/// let lw = panic_local_waker();
/// lw.wake(); // Will panic
/// ```
pub fn panic_local_waker() -> LocalWaker {
    unsafe { LocalWaker::new_unchecked(raw_panic_waker()) }
}

/// Get a global reference to a
/// [`LocalWaker`](futures_core::task::LocalWaker) referencing a singleton
/// instance of a [`LocalWaker`] which panics when woken.
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await, futures_api)]
/// use futures::task;
/// use futures_test::task::panic_local_waker_ref;
///
/// let lw = panic_local_waker_ref();
/// lw.wake(); // Will panic
/// ```
pub fn panic_local_waker_ref() -> &'static LocalWaker {
    thread_local! {
        static PANIC_WAKER_INSTANCE: UnsafeCell<LocalWaker> =
            UnsafeCell::new(panic_local_waker());
    }
    PANIC_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}
