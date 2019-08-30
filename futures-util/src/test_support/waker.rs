use std::task::{Waker, RawWaker, RawWakerVTable};

pub(crate) fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP_WAKER_VTABLE)) }
}

static NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    noop_waker_clone,
    noop_waker_rest,
    noop_waker_rest,
    noop_waker_rest
);

unsafe fn noop_waker_clone(_data: *const ()) -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_WAKER_VTABLE)
}

unsafe fn noop_waker_rest(_data: *const ()) {
}