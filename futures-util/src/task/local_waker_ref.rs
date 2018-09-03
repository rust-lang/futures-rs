#![allow(clippy::cast_ptr_alignment)] // clippy is too strict here

use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::Arc;
use std::task::{LocalWaker, Waker, Wake, UnsafeWake};

/// A [`LocalWaker`](::std::task::LocalWaker) that is only valid for a given lifetime.
///
/// Note: this type implements [`Deref<Target = LocalWaker>`](::std::ops::Deref),
/// so it can be used to get a `&LocalWaker`.
#[derive(Debug)]
pub struct LocalWakerRef<'a> {
    local_waker: LocalWaker,
    _marker: PhantomData<&'a ()>,
}

impl<'a> LocalWakerRef<'a> {
    /// Create a new [`LocalWakerRef`] from a [`LocalWaker`].
    ///
    /// Note: this function is safe, but it is generally only used
    /// from `unsafe` contexts that need to create a `LocalWaker`
    /// that is guaranteed not to outlive a particular lifetime.
    pub fn new(local_waker: LocalWaker) -> Self {
        LocalWakerRef {
            local_waker,
            _marker: PhantomData,
        }
    }
}

impl<'a> Deref for LocalWakerRef<'a> {
    type Target = LocalWaker;

    fn deref(&self) -> &LocalWaker {
        &self.local_waker
    }
}

// Pointers to this type below are really pointers to `Arc<W>`
struct ReferencedArc<W> {
    _marker: PhantomData<W>,
}

unsafe impl<W: Wake + 'static> UnsafeWake for ReferencedArc<W> {
    #[inline]
    unsafe fn clone_raw(&self) -> Waker {
        let me = self as *const ReferencedArc<W> as *const Arc<W>;
        Arc::clone(&*me).into()
    }

    #[inline]
    unsafe fn drop_raw(&self) {}

    #[inline]
    unsafe fn wake(&self) {
        let me = self as *const ReferencedArc<W> as *const Arc<W>;
        W::wake(&*me)
    }

    #[inline]
    unsafe fn wake_local(&self) {
        let me = self as *const ReferencedArc<W> as *const Arc<W>;
        W::wake_local(&*me)
    }
}

/// Creates a reference to a [`LocalWaker`](::std::task::LocalWaker)
/// from a local [`wake`](::std::task::Wake).
///
/// # Safety
///
/// This function requires that `wake` is "local" (created on the current thread).
/// The resulting [`LocalWaker`](::std::task::LocalWaker) will call
/// [`wake.wake_local()`](::std::task::Wake::wake_local)
/// when awoken, and will call [`wake.wake()`](::std::task::Wake::wake) if
/// awoken after being converted to a [`Waker`](::std::task::Waker).
#[inline]
pub unsafe fn local_waker_ref<W>(wake: &Arc<W>) -> LocalWakerRef<'_>
where
    W: Wake + 'static,
{
    let ptr = wake
        as *const Arc<W>
        as *const ReferencedArc<W>
        as *const dyn UnsafeWake
        as *mut dyn UnsafeWake;
    let local_waker = LocalWaker::new(NonNull::new_unchecked(ptr));
    LocalWakerRef::new(local_waker)
}

// Pointers to this type below are really pointers to `Arc<W>`,
struct NonlocalReferencedArc<W> {
    _marker: PhantomData<W>,
}

unsafe impl<W: Wake + 'static> UnsafeWake for NonlocalReferencedArc<W> {
    #[inline]
    unsafe fn clone_raw(&self) -> Waker {
        let me = self as *const NonlocalReferencedArc<W> as *const Arc<W>;
        Arc::clone(&*me).into()
    }

    #[inline]
    unsafe fn drop_raw(&self) {}

    #[inline]
    unsafe fn wake(&self) {
        let me = self as *const NonlocalReferencedArc<W> as *const Arc<W>;
        W::wake(&*me)
    }

    #[inline]
    unsafe fn wake_local(&self) {
        let me = self as *const NonlocalReferencedArc<W> as *const Arc<W>;
        W::wake(&*me)
    }
}

/// Creates a reference to a [`LocalWaker`](::std::task::LocalWaker)
/// from a non-local [`wake`](::std::task::Wake).
///
/// This function is similar to [`local_waker_ref()`], but does not require
/// that `wake` is local to the current thread. The resulting
/// [`LocalWaker`](::std::task::LocalWaker) will call
/// [`wake.wake()`](::std::task::Wake::wake) when awoken.
#[inline]
pub fn local_waker_ref_from_nonlocal<W>(wake: &Arc<W>) -> LocalWakerRef<'_>
where
    W: Wake + 'static,
{
    let ptr = wake
        as *const Arc<W>
        as *const NonlocalReferencedArc<W>
        as *const dyn UnsafeWake
        as *mut dyn UnsafeWake;
    let local_waker = unsafe { LocalWaker::new(NonNull::new_unchecked(ptr)) };
    LocalWakerRef::new(local_waker)
}
