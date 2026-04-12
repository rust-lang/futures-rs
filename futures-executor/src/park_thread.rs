use crate::enter::Enter;
use crate::park::{Park, ParkDuration, ParkPoll};
use futures_util::task::{waker_ref, ArcWake, WakerRef};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::task::Poll;
use std::thread;

/// Implements [Park] using [thread::park] to put the thread to sleep.
#[derive(Debug)]
pub struct ParkThread {
    // store a copy of TLS data here so `waker()` below can return a
    // reference to it
    notify: Arc<ThreadNotify>,
}

impl ParkThread {
    /// Create new `ParkThread` instance.
    pub fn new() -> Self {
        ParkThread { notify: CURRENT_THREAD_NOTIFY.with(Arc::clone) }
    }
}

impl Default for ParkThread {
    fn default() -> Self {
        ParkThread::new()
    }
}

impl Park for ParkThread {
    fn waker(&self) -> WakerRef<'_> {
        waker_ref(&self.notify)
    }

    fn park(&mut self, enter: &mut Enter, duration: ParkDuration) {
        // Wait for (and reset if it happend) a wakeup.
        if !self.poll(enter).is_ready() {
            // No wakeup occurred. It may occur now, right before parking,
            // but in that case the token made available by `unpark()`
            // is guaranteed to still be available and `park()` is a no-op.
            match duration {
                ParkDuration::Poll => (),
                ParkDuration::Block => thread::park(),
                ParkDuration::Timeout(duration) => thread::park_timeout(duration),
            }
        }
    }
}

impl ParkPoll for ParkThread {
    fn poll(&mut self, _enter: &mut Enter) -> Poll<()> {
        let unparked = CURRENT_THREAD_NOTIFY
            .with(|thread_notify| thread_notify.unparked.swap(false, Ordering::Acquire));

        if unparked {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

std::thread_local! {
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        thread: thread::current(),
        unparked: AtomicBool::new(false),
    });
}

#[derive(Debug)]
struct ThreadNotify {
    /// The (single) executor thread.
    thread: thread::Thread,
    /// A flag to ensure a wakeup (i.e. `unpark()`) is not "forgotten"
    /// before the next `park()`, which may otherwise happen if the code
    /// being executed as part of the future(s) being polled makes use of
    /// park / unpark calls of its own, i.e. we cannot assume that no other
    /// code uses park / unpark on the executing `thread`.
    unparked: AtomicBool,
}

impl ArcWake for ThreadNotify {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Make sure the wakeup is remembered until the next `park()`.
        let unparked = arc_self.unparked.swap(true, Ordering::Release);
        if !unparked {
            // If the thread has not been unparked yet, it must be done
            // now. If it was actually parked, it will run again,
            // otherwise the token made available by `unpark`
            // may be consumed before reaching `park()`, but `unparked`
            // ensures it is not forgotten.
            arc_self.thread.unpark();
        }
    }
}
