#![allow(clippy::cast_ptr_alignment)] // clippy is too strict here

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::task::{Waker, RawWaker, RawWakerVTable};
use super::ArcWake;

/// A [`Waker`](::std::task::Waker) that is only valid for a given lifetime.
///
/// Note: this type implements [`Deref<Target = Waker>`](::std::ops::Deref),
/// so it can be used to get a `&Waker`.
#[derive(Debug)]
pub struct WakerRef<'a> {
    waker: Waker,
    _marker: PhantomData<&'a ()>,
}

impl<'a> WakerRef<'a> {
    /// Create a new [`WakerRef`] from a [`Waker`].
    ///
    /// Note: this function is safe, but it is generally only used
    /// from `unsafe` contexts that need to create a `Waker`
    /// that is guaranteed not to outlive a particular lifetime.
    pub fn new(waker: Waker) -> Self {
        WakerRef {
            waker,
            _marker: PhantomData,
        }
    }
}

impl<'a> Deref for WakerRef<'a> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}

// Another reference vtable which doesn't do decrement the refcount on drop.
// However on clone it will create a vtable which equals a Waker, and on wake
// it will call the nonlocal wake function.
macro_rules! ref_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_raw::<$ty>,
            drop: noop,
            wake: wake_arc_raw::<$ty>,
        }
    };
}

macro_rules! owned_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_raw::<$ty>,
            drop: drop_arc_raw::<$ty>,
            wake: wake_arc_raw::<$ty>,
        }
    };
}

/// Creates a reference to a [`Waker`](::std::task::Waker)
/// from a local [`wake`](::std::task::Wake).
///
/// The resulting [`Waker`](::std::task::Waker) will call
/// [`wake.wake()`](::std::task::Wake::wake) if awoken.
#[inline]
pub fn waker_ref<W>(wake: &Arc<W>) -> WakerRef<'_>
where
    W: ArcWake
{
    // This uses the same mechanism as Arc::into_raw, without needing a reference.
    // This is potentially not stable
    let ptr = &*wake as &W as *const W as *const();

    let waker = unsafe {
        Waker::new_unchecked(RawWaker{
            data: ptr,
            vtable: ref_vtable!(W),
        })};
    WakerRef::new(waker)
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

unsafe fn clone_arc_raw<T: ArcWake>(data: *const()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker {
        data: data,
        vtable: owned_vtable!(T),
    }
}

unsafe fn drop_arc_raw<T: ArcWake>(data: *const()) {
    // Drop Arc
    let _: Arc<T> = Arc::from_raw(data as *const T);
}

unsafe fn wake_arc_raw<T: ArcWake>(data: *const()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(&arc); // TODO: If this panics, the refcount is too big
    let _ = Arc::into_raw(arc);
}