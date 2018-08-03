use futures_core::task::{LocalWaker, UnsafeWake, Wake, Waker};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::Arc;

/// An implementation of [`Wake`][futures_core::task::Wake] that panics when
/// woken.
///
/// # Examples
///
/// ```should_panic
/// #![feature(futures_api)]
/// use futures_test::task::{noop_context, wake};
///
/// let mut cx = noop_context();
/// let cx = &mut cx.with_waker(wake::Panic::local_waker_ref());
///
/// cx.waker().wake(); // Will panic
/// ```
#[derive(Debug)]
pub struct Panic {
    _reserved: (),
}

impl Panic {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _reserved: () }
    }

    fn unsafe_wake() -> NonNull<dyn UnsafeWake> {
        static mut INSTANCE: Panic = Panic { _reserved: () };
        unsafe { NonNull::new_unchecked(&mut INSTANCE as *mut dyn UnsafeWake) }
    }

    /// Create a new [`Waker`] referencing a singleton instance of [`Panic`].
    pub fn waker() -> Waker {
        unsafe { Waker::new(Self::unsafe_wake()) }
    }

    /// Create a new [`LocalWaker`] referencing a singleton instance of
    /// [`Panic`].
    pub fn local_waker() -> LocalWaker {
        unsafe { LocalWaker::new(Self::unsafe_wake()) }
    }

    /// Get a thread local reference to a [`LocalWaker`] referencing a singleton
    /// instance of [`Panic`].
    pub fn local_waker_ref() -> &'static LocalWaker {
        thread_local! {
            static LOCAL_WAKER_INSTANCE: UnsafeCell<LocalWaker> =
                UnsafeCell::new(Panic::local_waker());
        }
        LOCAL_WAKER_INSTANCE.with(|l| unsafe { &mut *l.get() })
    }
}

impl Default for Panic {
    fn default() -> Self {
        Self::new()
    }
}

impl Wake for Panic {
    fn wake(_arc_self: &Arc<Self>) {
        panic!("should not be woken")
    }
}

unsafe impl UnsafeWake for Panic {
    unsafe fn clone_raw(&self) -> Waker {
        Panic::waker()
    }

    unsafe fn drop_raw(&self) {}

    unsafe fn wake(&self) {
        panic!("should not be woken")
    }
}
