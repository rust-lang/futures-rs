use core::ptr::null;
use futures_core::task::{RawWaker, RawWakerVTable, Waker};

unsafe fn clone_panic_waker(_data: *const ()) -> RawWaker {
    raw_panic_waker()
}

unsafe fn noop(_data: *const ()) {}

unsafe fn wake_panic(_data: *const ()) {
    if !std::thread::panicking() {
        panic!("should not be woken");
    }
}

const PANIC_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_panic_waker, wake_panic, wake_panic, noop);

const fn raw_panic_waker() -> RawWaker {
    RawWaker::new(null(), &PANIC_WAKER_VTABLE)
}

/// Create a new [`Waker`](futures_core::task::Waker) which will
/// panic when `wake()` is called on it. The [`Waker`] can be converted
/// into a [`Waker`] which will behave the same way.
///
/// # Examples
///
/// ```should_panic
/// use futures_test::task::panic_waker;
///
/// let waker = panic_waker();
/// waker.wake(); // Will panic
/// ```
pub fn panic_waker() -> Waker {
    // FIXME: Since 1.46.0 we can use transmute in consts, allowing this function to be const.
    unsafe { Waker::from_raw(raw_panic_waker()) }
}

/// Get a global reference to a
/// [`Waker`](futures_core::task::Waker) referencing a singleton
/// instance of a [`Waker`] which panics when woken.
///
/// # Examples
///
/// ```should_panic
/// use futures_test::task::panic_waker_ref;
///
/// let waker = panic_waker_ref();
/// waker.wake_by_ref(); // Will panic
/// ```
pub fn panic_waker_ref() -> &'static Waker {
    struct SyncRawWaker(RawWaker);
    unsafe impl Sync for SyncRawWaker {}

    static PANIC_WAKER_INSTANCE: SyncRawWaker = SyncRawWaker(raw_panic_waker());

    // SAFETY: `Waker` is #[repr(transparent)] over its `RawWaker`.
    unsafe { &*(&PANIC_WAKER_INSTANCE.0 as *const RawWaker as *const Waker) }
}

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "should not be woken")]
    fn issue_2091_cross_thread_segfault() {
        let waker = std::thread::spawn(super::panic_waker_ref).join().unwrap();
        waker.wake_by_ref();
    }
}
