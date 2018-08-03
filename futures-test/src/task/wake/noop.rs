use futures_core::task::{LocalWaker, UnsafeWake, Wake, Waker};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::Arc;

/// An implementation of [`Wake`][futures_core::task::Wake] that does nothing
/// when woken.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures_test::task::{panic_context, wake};
///
/// let mut cx = panic_context();
/// let cx = &mut cx.with_waker(wake::Noop::local_waker_ref());
///
/// cx.waker().wake();
/// ```
#[derive(Debug)]
pub struct Noop {
    _reserved: (),
}

impl Noop {
    /// Create a new instance
    pub fn new() -> Self {
        Self { _reserved: () }
    }

    fn unsafe_wake() -> NonNull<dyn UnsafeWake> {
        static mut INSTANCE: Noop = Noop { _reserved: () };
        unsafe { NonNull::new_unchecked(&mut INSTANCE as *mut dyn UnsafeWake) }
    }

    /// Create a new [`Waker`] referencing a singleton instance of [`Noop`].
    pub fn waker() -> Waker {
        unsafe { Waker::new(Self::unsafe_wake()) }
    }

    /// Create a new [`LocalWaker`] referencing a singleton instance of
    /// [`Noop`].
    pub fn local_waker() -> LocalWaker {
        unsafe { LocalWaker::new(Self::unsafe_wake()) }
    }

    /// Get a thread local reference to a [`LocalWaker`] referencing a singleton
    /// instance of [`Noop`].
    pub fn local_waker_ref() -> &'static LocalWaker {
        thread_local! {
            static LOCAL_WAKER_INSTANCE: UnsafeCell<LocalWaker> =
                UnsafeCell::new(Noop::local_waker());
        }
        LOCAL_WAKER_INSTANCE.with(|l| unsafe { &mut *l.get() })
    }
}

impl Default for Noop {
    fn default() -> Self {
        Self::new()
    }
}

impl Wake for Noop {
    fn wake(_arc_self: &Arc<Self>) {}
}

unsafe impl UnsafeWake for Noop {
    unsafe fn clone_raw(&self) -> Waker {
        Noop::waker()
    }

    unsafe fn drop_raw(&self) {}

    unsafe fn wake(&self) {}
}
