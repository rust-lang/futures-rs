use futures_core::task::{LocalWaker, UnsafeWake, Wake, Waker};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::Arc;

/// An implementation of [`Wake`](futures_core::task::Wake) that panics when
/// woken.
///
/// # Examples
///
/// ```should_panic
/// #![feature(futures_api)]
/// use futures_test::task::{noop_context, panic_local_waker_ref};
///
/// let mut lw = noop_context();
/// let lw = &mut lw.with_waker(panic_local_waker_ref());
///
/// lw.waker().wake(); // Will panic
/// ```
#[derive(Debug)]
pub struct PanicWake {
    _reserved: (),
}

impl PanicWake {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _reserved: () }
    }
}

impl Default for PanicWake {
    fn default() -> Self {
        Self::new()
    }
}

impl Wake for PanicWake {
    fn wake(_arc_self: &Arc<Self>) {
        panic!("should not be woken")
    }
}

unsafe impl UnsafeWake for PanicWake {
    unsafe fn clone_raw(&self) -> Waker {
        panic_waker()
    }

    unsafe fn drop_raw(&self) {}

    unsafe fn wake(&self) {
        panic!("should not be woken")
    }
}

fn panic_unsafe_wake() -> NonNull<dyn UnsafeWake> {
    static mut INSTANCE: PanicWake = PanicWake { _reserved: () };
    unsafe { NonNull::new_unchecked(&mut INSTANCE as *mut dyn UnsafeWake) }
}

fn panic_waker() -> Waker {
    unsafe { Waker::new(panic_unsafe_wake()) }
}

/// Create a new [`LocalWaker`](futures_core::task::LocalWaker) referencing
/// a singleton instance of [`PanicWake`].
pub fn panic_local_waker() -> LocalWaker {
    unsafe { LocalWaker::new(panic_unsafe_wake()) }
}

/// Get a thread local reference to a
/// [`LocalWaker`](futures_core::task::LocalWaker) referencing a singleton
/// instance of [`PanicWake`].
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await, futures_api)]
/// use futures::task;
/// use futures_test::task::{panic_local_waker_ref, panic_spawner_mut};
///
/// let mut lw = task::Context::new(
///     panic_local_waker_ref(),
///     panic_spawner_mut(),
/// );
///
/// lw.waker().wake(); // Will panic
/// ```
pub fn panic_local_waker_ref() -> &'static LocalWaker {
    thread_local! {
        static LOCAL_WAKER_INSTANCE: UnsafeCell<LocalWaker> =
            UnsafeCell::new(panic_local_waker());
    }
    LOCAL_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}
