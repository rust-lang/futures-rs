use std::prelude::v1::*;

use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::{AtomicUsize, Ordering};

use {Future, Stream, Async};
use super::{NotifyHandle, Spawn, Notify, UnsafeNotify};
pub use super::core::{BorrowedUnpark, TaskUnpark};

mod data;
pub use self::data::*;

impl<F: Future> Spawn<F> {
    /// Waits for the internal future to complete, blocking this thread's
    /// execution until it does.
    ///
    /// This function will call `poll_future` in a loop, waiting for the future
    /// to complete. When a future cannot make progress it will use
    /// `thread::park` to block the current thread.
    pub fn wait_future(&mut self) -> Result<F::Item, F::Error> {
        ThreadNotify::with_current(|notify| {

            loop {
                match self.poll_future_notify(notify, 0)? {
                    Async::Pending => notify.park(),
                    Async::Ready(e) => return Ok(e),
                }
            }
        })
    }
}

impl<S: Stream> Spawn<S> {
    /// Like `wait_future`, except only waits for the next element to arrive on
    /// the underlying stream.
    pub fn wait_stream(&mut self) -> Option<Result<S::Item, S::Error>> {
        ThreadNotify::with_current(|notify| {

            loop {
                match self.poll_stream_notify(notify, 0) {
                    Ok(Async::Pending) => notify.park(),
                    Ok(Async::Ready(Some(e))) => return Some(Ok(e)),
                    Ok(Async::Ready(None)) => return None,
                    Err(e) => return Some(Err(e)),
                }
            }
        })
    }
}

// ===== ThreadNotify =====

struct ThreadNotify {
    state: AtomicUsize,
    mutex: Mutex<()>,
    condvar: Condvar,
}

const IDLE: usize = 0;
const NOTIFY: usize = 1;
const SLEEP: usize = 2;

thread_local! {
    static CURRENT_THREAD_NOTIFY: Arc<ThreadNotify> = Arc::new(ThreadNotify {
        state: AtomicUsize::new(IDLE),
        mutex: Mutex::new(()),
        condvar: Condvar::new(),
    });
}

impl ThreadNotify {
    fn with_current<F, R>(f: F) -> R
        where F: FnOnce(&Arc<ThreadNotify>) -> R,
    {
        CURRENT_THREAD_NOTIFY.with(|notify| f(notify))
    }

    fn park(&self) {
        // If currently notified, then we skip sleeping. This is checked outside
        // of the lock to avoid acquiring a mutex if not necessary.
        match self.state.compare_and_swap(NOTIFY, IDLE, Ordering::SeqCst) {
            NOTIFY => return,
            IDLE => {},
            _ => unreachable!(),
        }

        // The state is currently idle, so obtain the lock and then try to
        // transition to a sleeping state.
        let mut m = self.mutex.lock().unwrap();

        // Transition to sleeping
        match self.state.compare_and_swap(IDLE, SLEEP, Ordering::SeqCst) {
            NOTIFY => {
                // Notified before we could sleep, consume the notification and
                // exit
                self.state.store(IDLE, Ordering::SeqCst);
                return;
            }
            IDLE => {},
            _ => unreachable!(),
        }

        // Loop until we've been notified
        loop {
            m = self.condvar.wait(m).unwrap();

            // Transition back to idle, loop otherwise
            if NOTIFY == self.state.compare_and_swap(NOTIFY, IDLE, Ordering::SeqCst) {
                return;
            }
        }
    }
}

impl Notify for ThreadNotify {
    fn notify(&self, _unpark_id: usize) {
        // First, try transitioning from IDLE -> NOTIFY, this does not require a
        // lock.
        match self.state.compare_and_swap(IDLE, NOTIFY, Ordering::SeqCst) {
            IDLE | NOTIFY => return,
            SLEEP => {}
            _ => unreachable!(),
        }

        // The other half is sleeping, this requires a lock
        let _m = self.mutex.lock().unwrap();

        // Transition from SLEEP -> NOTIFY
        match self.state.compare_and_swap(SLEEP, NOTIFY, Ordering::SeqCst) {
            SLEEP => {}
            _ => return,
        }

        // Wakeup the sleeper
        self.condvar.notify_one();
    }
}

// Safe implementation of `UnsafeNotify` for `Arc` in the standard library.
//
// Note that this is a very unsafe implementation! The crucial pieces is that
// these two values are considered equivalent:
//
// * Arc<T>
// * *const ArcWrapped<T>
//
// We don't actually know the layout of `ArcWrapped<T>` as it's an
// implementation detail in the standard library. We can work, though, by
// casting it through and back an `Arc<T>`.
//
// This also means that you won't actually fine `UnsafeNotify for Arc<T>`
// because it's the wrong level of indirection. These methods are sort of
// receiving Arc<T>, but not an owned version. It's... complicated. We may be
// one of the first users of unsafe trait objects!

struct ArcWrapped<T>(PhantomData<T>);

impl<T: Notify + 'static> Notify for ArcWrapped<T> {
    fn notify(&self, id: usize) {
        unsafe {
            let me: *const ArcWrapped<T> = self;
            T::notify(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>),
                      id)
        }
    }

    fn clone_id(&self, id: usize) -> usize {
        unsafe {
            let me: *const ArcWrapped<T> = self;
            T::clone_id(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>),
                        id)
        }
    }

    fn drop_id(&self, id: usize) {
        unsafe {
            let me: *const ArcWrapped<T> = self;
            T::drop_id(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>),
                       id)
        }
    }
}

unsafe impl<T: Notify + 'static> UnsafeNotify for ArcWrapped<T> {
    unsafe fn clone_raw(&self) -> NotifyHandle {
        let me: *const ArcWrapped<T> = self;
        let arc = (*(&me as *const *const ArcWrapped<T> as *const Arc<T>)).clone();
        NotifyHandle::from(arc)
    }

    unsafe fn drop_raw(&self) {
        let mut me: *const ArcWrapped<T> = self;
        let me = &mut me as *mut *const ArcWrapped<T> as *mut Arc<T>;
        ptr::drop_in_place(me);
    }
}

impl<T> From<Arc<T>> for NotifyHandle
    where T: Notify + 'static,
{
    fn from(rc: Arc<T>) -> NotifyHandle {
        unsafe {
            let ptr = mem::transmute::<Arc<T>, *mut ArcWrapped<T>>(rc);
            NotifyHandle::new(ptr)
        }
    }
}
