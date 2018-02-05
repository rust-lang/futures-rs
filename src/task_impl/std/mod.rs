use std::prelude::v1::*;

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::{Arc, Mutex, Condvar, Once, ONCE_INIT};
use std::sync::atomic::{AtomicUsize, Ordering};

use {Future, Stream, Sink, Async, AsyncSink};
use super::core;
use super::{BorrowedTask, NotifyHandle, Spawn, spawn, Notify, UnsafeNotify};
pub use super::core::{BorrowedUnpark, TaskUnpark};

mod unpark_mutex;
pub use self::unpark_mutex::UnparkMutex;

mod data;
pub use self::data::*;

pub use task_impl::core::init;

thread_local!(static CURRENT_TASK: Cell<*mut u8> = Cell::new(ptr::null_mut()));

static INIT: Once = ONCE_INIT;

pub fn get_ptr() -> Option<*mut u8> {
    // Since this condition will always return true when TLS task storage is
    // used (the default), the branch predictor will be able to optimize the
    // branching and a dynamic dispatch will be avoided, which makes the
    // compiler happier.
    if core::is_get_ptr(0x1) {
        Some(CURRENT_TASK.with(|c| c.get()))
    } else {
        core::get_ptr()
    }
}

fn tls_slot() -> *const Cell<*mut u8> {
    CURRENT_TASK.with(|c| c as *const _)
}

pub fn set<'a, F, R>(task: &BorrowedTask<'a>, f: F) -> R
    where F: FnOnce() -> R
{
    // Lazily initialize the get / set ptrs
    //
    // Note that we won't actually use these functions ever, we'll instead be
    // testing the pointer's value elsewhere and calling our own functions.
    INIT.call_once(|| unsafe {
        let get = mem::transmute::<usize, _>(0x1);
        let set = mem::transmute::<usize, _>(0x2);
        init(get, set);
    });

    // Same as above.
    if core::is_get_ptr(0x1) {
        struct Reset(*const Cell<*mut u8>, *mut u8);

        impl Drop for Reset {
            #[inline]
            fn drop(&mut self) {
                unsafe {
                    (*self.0).set(self.1);
                }
            }
        }

        unsafe {
            let slot = tls_slot();
            let _reset = Reset(slot, (*slot).get());
            (*slot).set(task as *const _ as *mut u8);
            f()
        }
    } else {
        core::set(task, f)
    }
}

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
                    Async::NotReady => notify.park(),
                    Async::Ready(e) => return Ok(e),
                }
            }
        })
    }

    /// A specialized function to request running a future to completion on the
    /// specified executor.
    ///
    /// This function only works for futures whose item and error types are `()`
    /// and also implement the `Send` and `'static` bounds. This will submit
    /// units of work (instances of `Run`) to the `exec` argument provided
    /// necessary to drive the future to completion.
    ///
    /// When the future would block, it's arranged that when the future is again
    /// ready it will submit another unit of work to the `exec` provided. This
    /// will happen in a loop until the future has completed.
    ///
    /// This method is not appropriate for all futures, and other kinds of
    /// executors typically provide a similar function with perhaps relaxed
    /// bounds as well.
    ///
    /// Note that this method is likely to be deprecated in favor of the
    /// `futures::Executor` trait and `execute` method, but if this'd cause
    /// difficulty for you please let us know!
    pub fn execute(self, exec: Arc<Executor>)
        where F: Future<Item=(), Error=()> + Send + 'static,
    {
        exec.clone().execute(Run {
            // Ideally this method would be defined directly on
            // `Spawn<BoxFuture<(), ()>>` so we wouldn't have to box here and
            // it'd be more explicit, but unfortunately that currently has a
            // link error on nightly: rust-lang/rust#36155
            spawn: spawn(Box::new(self.into_inner())),
            inner: Arc::new(RunInner {
                exec: exec,
                mutex: UnparkMutex::new()
            }),
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
                    Ok(Async::NotReady) => notify.park(),
                    Ok(Async::Ready(Some(e))) => return Some(Ok(e)),
                    Ok(Async::Ready(None)) => return None,
                    Err(e) => return Some(Err(e)),
                }
            }
        })
    }
}

impl<S: Sink> Spawn<S> {
    /// Blocks the current thread until it's able to send `value` on this sink.
    ///
    /// This function will send the `value` on the sink that this task wraps. If
    /// the sink is not ready to send the value yet then the current thread will
    /// be blocked until it's able to send the value.
    pub fn wait_send(&mut self, mut value: S::SinkItem)
                     -> Result<(), S::SinkError> {
        ThreadNotify::with_current(|notify| {

            loop {
                value = match self.start_send_notify(value, notify, 0)? {
                    AsyncSink::NotReady(v) => v,
                    AsyncSink::Ready => return Ok(()),
                };
                notify.park();
            }
        })
    }

    /// Blocks the current thread until it's able to flush this sink.
    ///
    /// This function will call the underlying sink's `poll_complete` method
    /// until it returns that it's ready, proxying out errors upwards to the
    /// caller if one occurs.
    ///
    /// The thread will be blocked until `poll_complete` returns that it's
    /// ready.
    pub fn wait_flush(&mut self) -> Result<(), S::SinkError> {
        ThreadNotify::with_current(|notify| {

            loop {
                if self.poll_flush_notify(notify, 0)?.is_ready() {
                    return Ok(())
                }
                notify.park();
            }
        })
    }

    /// Blocks the current thread until it's able to close this sink.
    ///
    /// This function will close the sink that this task wraps. If the sink
    /// is not ready to be close yet, then the current thread will be blocked
    /// until it's closed.
    pub fn wait_close(&mut self) -> Result<(), S::SinkError> {
        ThreadNotify::with_current(|notify| {

            loop {
                if self.close_notify(notify, 0)?.is_ready() {
                    return Ok(())
                }
                notify.park();
            }
        })
    }
}

/// A trait representing requests to poll futures.
///
/// This trait is an argument to the `Spawn::execute` which is used to run a
/// future to completion. An executor will receive requests to run a future and
/// an executor is responsible for ensuring that happens in a timely fashion.
///
/// Note that this trait is likely to be deprecated and/or renamed to avoid
/// clashing with the `future::Executor` trait. If you've got a use case for
/// this or would like to comment on the name please let us know!
pub trait Executor: Send + Sync + 'static {
    /// Requests that `Run` is executed soon on the given executor.
    fn execute(&self, r: Run);
}

/// Units of work submitted to an `Executor`, currently only created
/// internally.
pub struct Run {
    spawn: Spawn<Box<Future<Item = (), Error = ()> + Send>>,
    inner: Arc<RunInner>,
}

struct RunInner {
    mutex: UnparkMutex<Run>,
    exec: Arc<Executor>,
}

impl Run {
    /// Actually run the task (invoking `poll` on its future) on the current
    /// thread.
    pub fn run(self) {
        let Run { mut spawn, inner } = self;

        // SAFETY: the ownership of this `Run` object is evidence that
        // we are in the `POLLING`/`REPOLL` state for the mutex.
        unsafe {
            inner.mutex.start_poll();

            loop {
                match spawn.poll_future_notify(&inner, 0) {
                    Ok(Async::NotReady) => {}
                    Ok(Async::Ready(())) |
                    Err(()) => return inner.mutex.complete(),
                }
                let run = Run { spawn: spawn, inner: inner.clone() };
                match inner.mutex.wait(run) {
                    Ok(()) => return,            // we've waited
                    Err(r) => spawn = r.spawn,   // someone's notified us
                }
            }
        }
    }
}

impl fmt::Debug for Run {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Run")
         .field("contents", &"...")
         .finish()
    }
}

impl Notify for RunInner {
    fn notify(&self, _id: usize) {
        match self.mutex.notify() {
            Ok(run) => self.exec.execute(run),
            Err(()) => {}
        }
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

/// A concurrent set which allows for the insertion of `usize` values.
///
/// `EventSet`s are used to communicate precise information about the event(s)
/// that triggered a task notification. See `task::with_unpark_event` for details.
#[deprecated(since="0.1.18", note = "recommended to use `FuturesUnordered` instead")]
pub trait EventSet: Send + Sync + 'static {
    /// Insert the given ID into the set
    fn insert(&self, id: usize);
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
