use core::mem;
use core::task::{Waker, RawWaker, RawWakerVTable};
use alloc::sync::Arc;

/// A way of waking up a specific task.
///
/// By implementing this trait, types that are expected to be wrapped in an `Arc`
/// can be converted into `Waker` objects.
/// Those Wakers can be used to signal executors that a task it owns
/// is ready to be `poll`ed again.
// Note: Send + Sync required because `Arc<T>` doesn't automatically imply
// those bounds, but `Waker` implements them.
pub trait ArcWake: Send + Sync {
    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based `Waker`.
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake` should place
    /// the associated task onto this queue.
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self)
    }

    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based `Waker`.
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake_by_ref` should place
    /// the associated task onto this queue.
    ///
    /// This function is similar to `wake`, but must not consume the provided data
    /// pointer.
    fn wake_by_ref(arc_self: &Arc<Self>);

    /// Creates a `Waker` from an Arc<T>, if T implements `ArcWake`.
    ///
    /// If `wake()` is called on the returned `Waker`,
    /// the `wake()` function that is defined inside this trait will get called.
    fn into_waker(self: Arc<Self>) -> Waker where Self: Sized
    {
        let ptr = Arc::into_raw(self) as *const ();

        unsafe {
            Waker::from_raw(RawWaker::new(ptr, waker_vtable!(Self)))
        }
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

unsafe fn drop_arc_raw<T: ArcWake>(data: *const ()) {
    drop(Arc::<T>::from_raw(data as *const T))
}

// used by `waker_ref`
pub(super) unsafe fn wake_arc_raw<T: ArcWake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(arc);
}

// used by `waker_ref`
pub(super) unsafe fn wake_by_ref_arc_raw<T: ArcWake>(data: *const ()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake_by_ref(&arc);
    mem::forget(arc);
}
