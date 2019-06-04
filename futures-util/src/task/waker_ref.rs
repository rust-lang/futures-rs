#![allow(clippy::cast_ptr_alignment)] // clippy is too strict here

use super::arc_wake::{ArcWake, clone_arc_raw, wake_by_ref_arc_raw};
use alloc::sync::Arc;
use core::marker::PhantomData;
use core::ops::Deref;
use core::task::{Waker, RawWaker, RawWakerVTable};

// TODO: The link to Waker below points to futures::task::Waker and not to std. Is that a
// bug in rustdoc?
//
/// A [`Waker`](::std::task::Waker) that is only valid for a given lifetime.
///
/// Note: this type implements [`Deref<Target = Waker>`](::std::ops::Deref),
/// so it can be used to get a `&Waker`.
#[derive(Debug)]
pub struct WakerRef<'a> {
    waker: Waker,
    _marker: PhantomData<&'a ()>,
}

impl WakerRef<'_> {
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

impl Deref for WakerRef<'_> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}

#[inline]
unsafe fn noop(_data: *const ()) {}

unsafe fn wake_unreachable(_data: *const ()) {
    // With only a reference, calling `wake_arc_raw()` would be unsound,
    // since the `WakerRef` didn't increment the refcount of the `ArcWake`,
    // and `wake_arc_raw` would *decrement* it.
    //
    // This should never be reachable, since `WakerRef` only provides a `Deref`
    // to the inner `Waker`.
    //
    // Still, safer to panic here than to call `wake_arc_raw`.
    unreachable!("WakerRef::wake");
}

/// Creates a reference to a [`Waker`](::std::task::Waker)
/// from a local [`ArcWake`].
///
/// The resulting [`Waker`](::std::task::Waker) will call
/// [`ArcWake.wake()`](ArcWake::wake) if awoken.
#[inline]
pub fn waker_ref<W>(wake: &Arc<W>) -> WakerRef<'_>
where
    W: ArcWake
{
    // This uses the same mechanism as Arc::into_raw, without needing a reference.
    // This is potentially not stable
    let ptr = &*wake as &W as *const W as *const ();

    // Similar to `waker_vtable`, but with a no-op `drop` function.
    // Clones of the resulting `RawWaker` will still be dropped normally.
    let vtable = &RawWakerVTable::new(
        clone_arc_raw::<W>,
        wake_unreachable,
        wake_by_ref_arc_raw::<W>,
        noop,
    );

    let waker = unsafe {
        Waker::from_raw(RawWaker::new(ptr, vtable))
    };
    WakerRef::new(waker)
}
