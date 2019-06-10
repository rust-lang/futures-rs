use crate::enter::Enter;
use crate::park::{Park, ParkDuration};
use std::sync::Arc;
use std::thread;
use futures_util::task::{waker_ref, ArcWake, WakerRef};

/// Implements [`Park`][p] using [`thread::park`] to put the thread to
/// sleep.
///
/// [`thread::park`]: https://doc.rust-lang.org/std/thread/fn.park.html
/// [p]: ../park/trait.Park.html
#[derive(Debug)]
pub struct ParkThread {
    // store a copy of TLS data here to `waker()` below can return a
    // reference to it
    notify: Arc<ThreadNotify>
}

impl ParkThread {
    /// Create new `ParkThread` instance.
    pub fn new() -> Self {
        ParkThread {
            notify: CURRENT_THREAD_NOTIFY.with(Arc::clone),
        }
    }
}

impl Default for ParkThread {
    fn default() -> Self {
        ParkThread::new()
    }
}

impl Park for ParkThread {
    type Error = std::convert::Infallible;

    fn waker(&self) -> WakerRef<'_> {
        waker_ref(&self.notify)
    }

    fn park(&mut self, _enter: &mut Enter, duration: ParkDuration) -> Result<(), Self::Error> {
        match duration {
            ParkDuration::Poll => (),
            ParkDuration::Block => thread::park(),
            ParkDuration::Timeout(duration) => thread::park_timeout(duration),
        }
        Ok(())
    }
}

thread_local! {
    // allocate only once per thread
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        thread: thread::current(),
    });
}

#[derive(Debug)]
struct ThreadNotify {
    thread: thread::Thread,
}

impl ArcWake for ThreadNotify {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.thread.unpark();
    }
}
