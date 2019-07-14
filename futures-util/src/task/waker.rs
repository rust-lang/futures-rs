use super::arc_wake::ArcWake;
use core::mem;
use core::task::{Waker, RawWaker, RawWakerVTable};
use alloc::sync::Arc;

/// Creates a [`Waker`] from an `Arc<impl ArcWake>`.
///
/// The returned [`Waker`] will call
/// [`ArcWake.wake()`](ArcWake::wake) if awoken.
pub fn waker<W>(wake: Arc<W>) -> Waker
where
    W: ArcWake,
{
    let ptr = Arc::into_raw(wake) as *const ();

    unsafe {
        Waker::from_raw(RawWaker::new(ptr, waker_vtable!(W)))
    }
}

// FIXME: panics on Arc::clone / refcount changes could wreak havoc on the
// code here. We should guard against this by aborting.

unsafe fn increase_refcount<T: ArcWake>(data: *const ()) {
    // Retain Arc by creating a copy
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    let arc_clone = arc.clone();
    // Forget the Arcs again, so that the refcount isn't decrased
    mem::forget(arc);
    mem::forget(arc_clone);
}

// used by `waker_ref`
pub(super) unsafe fn clone_arc_raw<T: ArcWake>(data: *const ()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker::new(data, waker_vtable!(T))
}

unsafe fn wake_arc_raw<T: ArcWake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(arc);
}

// used by `waker_ref`
pub(super) unsafe fn wake_by_ref_arc_raw<T: ArcWake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake_by_ref(&arc);
    mem::forget(arc);
}

unsafe fn drop_arc_raw<T: ArcWake>(data: *const ()) {
    drop(Arc::<T>::from_raw(data as *const T))
}
