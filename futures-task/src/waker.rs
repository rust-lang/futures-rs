use super::Wake;
use alloc::sync::Arc;
use core::mem;
use core::task::{RawWaker, RawWakerVTable, Waker};

pub(super) fn waker_vtable<W: Wake>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_arc_raw::<W>,
        wake_by_ref_arc_raw::<W>,
        drop_arc_raw::<W>,
    )
}

/// Creates a [`Waker`] from an `Arc<impl Wake>`.
///
/// The returned [`Waker`] will call
/// [`Wake.wake()`](Wake::wake) if awoken.
pub fn waker<W>(wake: Arc<W>) -> Waker
where
    W: Wake + Send + Sync + 'static,
{
    wake.into()
}

// FIXME: panics on Arc::clone / refcount changes could wreak havoc on the
// code here. We should guard against this by aborting.

#[allow(clippy::redundant_clone)] // The clone here isn't actually redundant.
unsafe fn increase_refcount<T: Wake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(Arc::<T>::from_raw(data as *const T));
    // Now increase refcount, but don't drop new refcount either
    let _arc_clone: mem::ManuallyDrop<_> = arc.clone();
}

// used by `waker_ref`
unsafe fn clone_arc_raw<T: Wake>(data: *const ()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker::new(data, waker_vtable::<T>())
}

unsafe fn wake_arc_raw<T: Wake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    Wake::wake(arc);
}

// used by `waker_ref`
unsafe fn wake_by_ref_arc_raw<T: Wake>(data: *const ()) {
    // Retain Arc, but don't touch refcount by wrapping in ManuallyDrop
    let arc = mem::ManuallyDrop::new(Arc::<T>::from_raw(data as *const T));
    Wake::wake_by_ref(&arc);
}

unsafe fn drop_arc_raw<T: Wake>(data: *const ()) {
    drop(Arc::<T>::from_raw(data as *const T))
}
