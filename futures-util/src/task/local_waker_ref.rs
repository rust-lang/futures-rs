#![allow(clippy::cast_ptr_alignment)] // clippy is too strict here

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::task::{LocalWaker, ArcWake, RawWaker, RawWakerVTable};

// TODO: Maybe it's better to rename this type, e.g. to LocalArcWakerRef

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

/// Creates the VTable for the LocalWaker in the reference
/// This LocalWaker does not increase the refcount of the Arc, since it
/// assumes it is still alive. Therefore it does not need to do anything on drop.
/// When a clone is created, this will increase the refcount and replace the
/// vtable with one that releases the refcount on drop.
macro_rules! local_ref_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_local_raw::<$ty>,
            drop_fn: noop,
            wake: wake_arc_local_raw::<$ty>,
            into_waker: into_waker_raw::<$ty>,
        }
    };
}

// Another reference vtable which doesn't do decrement the refcount on drop.
// However on clone it will create a vtable which equals a Waker, and on wake
// it will call the nonlocal wake function.
macro_rules! nonlocal_ref_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_nonlocal_raw::<$ty>,
            drop_fn: noop,
            wake: wake_arc_nonlocal_raw::<$ty>,
            into_waker: into_waker_raw::<$ty>,
        }
    };
}

macro_rules! local_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_local_raw::<$ty>,
            drop_fn: drop_arc_raw::<$ty>,
            wake: wake_arc_local_raw::<$ty>,
            into_waker: into_waker_raw::<$ty>,
        }
    };
}

macro_rules! nonlocal_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_nonlocal_raw::<$ty>,
            drop_fn: drop_arc_raw::<$ty>,
            wake: wake_arc_nonlocal_raw::<$ty>,
            into_waker: into_waker_raw::<$ty>,
        }
    };
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
    W: ArcWake
{
    // This uses the same mechanism as Arc::into_raw, without needing a reference.
    // This is potentially not stable
    let ptr = &*wake as &W as *const W as *const();

    let local_waker = LocalWaker::new_unchecked(RawWaker{
        data: ptr,
        vtable: local_ref_vtable!(W),
    });
    LocalWakerRef::new(local_waker)
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
    W: ArcWake
{
    // This uses the same mechanism as Arc::into_raw, without needing a reference.
    // This is potentially not stable
    let ptr = &*wake as &W as *const W as *const();

    let local_waker = unsafe {
        LocalWaker::new_unchecked(RawWaker{
            data: ptr,
            vtable: nonlocal_ref_vtable!(W),
        })
    };
    LocalWakerRef::new(local_waker)
}

unsafe fn noop(_data: *const()) {
}

unsafe fn increase_refcount<T: ArcWake>(data: *const()) {
    // Retain Arc by creating a copy
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    let arc_clone = arc.clone();
    // Forget the Arcs again, so that the refcount isn't decrased
    let _ = Arc::into_raw(arc);
    let _ = Arc::into_raw(arc_clone);
}

unsafe fn clone_arc_nonlocal_raw<T: ArcWake>(data: *const()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker {
        data: data,
        vtable: nonlocal_vtable!(T),
    }
}

unsafe fn clone_arc_local_raw<T: ArcWake>(data: *const()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker {
        data: data,
        vtable: local_vtable!(T),
    }
}

unsafe fn drop_arc_raw<T: ArcWake>(data: *const()) {
    // Drop Arc
    let _: Arc<T> = Arc::from_raw(data as *const T);
}

unsafe fn wake_arc_local_raw<T: ArcWake>(data: *const()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake_local(&arc); // TODO: If this panics, the refcount is too big
    let _ = Arc::into_raw(arc);
}

unsafe fn wake_arc_nonlocal_raw<T: ArcWake>(data: *const()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(&arc); // TODO: If this panics, the refcount is too big
    let _ = Arc::into_raw(arc);
}

unsafe fn into_waker_raw<T: ArcWake>(data: *const ()) -> Option<RawWaker> {
    Some(RawWaker {
        data: data,
        vtable: nonlocal_vtable!(T),
    })
}
