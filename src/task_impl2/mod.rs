#![allow(warnings)]

use std::prelude::v1::*;

use std::cell::Cell;
use std::fmt;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool, AtomicUsize, ATOMIC_USIZE_INIT};
use std::thread;

use {Poll, Future, Async, Stream, Sink, StartSend, AsyncSink};
use future::BoxFuture;

mod atomic_task;
pub use self::atomic_task::AtomicTask;

mod data;
pub use self::data::LocalKey;

struct BorrowedTask<'a> {
    unpark: &'a Arc<Unpark>,
    unpark_id: u64,
    // Task-local storage
    map: &'a data::LocalMap,
}

thread_local!(static CURRENT_TASK: Cell<*const BorrowedTask<'static>> = {
    Cell::new(0 as *const _)
});

fn set<'a, F, R>(task: &BorrowedTask<'a>, f: F) -> R
    where F: FnOnce() -> R
{
    struct Reset(*const BorrowedTask<'static>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT_TASK.with(|c| c.set(self.0));
        }
    }

    CURRENT_TASK.with(move |c| {
        let _reset = Reset(c.get());
        let task = unsafe {
            mem::transmute::<&BorrowedTask<'a>,
                             *const BorrowedTask<'static>>(task)
        };
        c.set(task);
        f()
    })
}

fn with<F: FnOnce(&BorrowedTask) -> R, R>(f: F) -> R {
    let task = CURRENT_TASK.with(|c| c.get());
    assert!(!task.is_null(), "no Task is currently running");
    unsafe {
        f(&*task)
    }
}

/// A handle to a "task", which represents a single lightweight "thread" of
/// execution driving a future to completion.
///
/// In general, futures are composed into large units of work, which are then
/// spawned as tasks onto an *executor*. The executor is responsible for polling
/// the future as notifications arrive, until the future terminates.
///
/// This is obtained by the `task::park` function.
pub struct Task {
    unpark: Arc<Unpark>,
    unpark_id: u64,
}

impl Clone for Task {
    fn clone(&self) -> Task {
        self.unpark.ref_inc(self.unpark_id);

        Task {
            unpark: self.unpark.clone(),
            unpark_id: self.unpark_id,
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        self.unpark.ref_dec(self.unpark_id);
    }
}

fn _assert_kinds() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Task>();
}

/// Returns a handle to the current task to call `unpark` at a later date.
///
/// This function is similar to the standard library's `thread::park` function
/// except that it won't block the current thread but rather the current future
/// that is being executed.
///
/// The returned handle implements the `Send` and `'static` bounds and may also
/// be cheaply cloned. This is useful for squirreling away the handle into a
/// location which is then later signaled that a future can make progress.
///
/// Implementations of the `Future` trait typically use this function if they
/// would otherwise perform a blocking operation. When something isn't ready
/// yet, this `park` function is called to acquire a handle to the current
/// task, and then the future arranges it such that when the block operation
/// otherwise finishes (perhaps in the background) it will `unpark` the returned
/// handle.
///
/// It's sometimes necessary to pass extra information to the task when
/// unparking it, so that the task knows something about *why* it was woken. See
/// the `with_unpark_event` for details on how to do this.
///
/// # Panics
///
/// This function will panic if a task is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
pub fn park() -> Task {
    with(|borrowed| {
        Task {
            unpark: borrowed.unpark.clone(),
            unpark_id: borrowed.unpark_id,
        }
    })
}

impl Task {
    /// Indicate that the task should attempt to poll its future in a timely
    /// fashion.
    ///
    /// It's typically guaranteed that, for each call to `unpark`, `poll` will
    /// be called at least once subsequently (unless the task has terminated).
    /// If the task is currently polling its future when `unpark` is called, it
    /// must poll the future *again* afterwards, ensuring that all relevant
    /// events are eventually observed by the future.
    pub fn unpark(&self) {
        self.unpark.unpark(self.unpark_id);
    }

    /// Returns `true` when called from within the context of the task. In
    /// other words, the task is currently running on the thread calling the
    /// function.
    pub fn is_current(&self) -> bool {
        with(|current| {
            if current.unpark_id != self.unpark_id {
                return false;
            }

            if let Some(eq) = self.unpark.is_current() {
                // Handles legacy task system...
                eq
            } else {
                let a = &**current.unpark as *const Unpark;
                let b = &*self.unpark as *const Unpark;

                if a != b {
                    return false;
                }

                true
            }
        })
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Task")
            // TODO: Are there any fields to show?
            .finish()
    }
}

/// Representation of a spawned future/stream.
///
/// This object is returned by the `spawn` function in this module. This
/// represents a "fused task and future", storing all necessary pieces of a task
/// and owning the top-level future that's being driven as well.
///
/// A `Spawn` can be poll'd for completion or execution of the current thread
/// can be blocked indefinitely until a notification arrives. This can be used
/// with either futures or streams, with different methods being available on
/// `Spawn` depending which is used.
pub struct Spawn<T> {
    obj: T,
    data: data::LocalMap,
}

/// Spawns a new future, returning the fused future and task.
///
/// This function is the termination endpoint for running futures. This method
/// will conceptually allocate a new task to run the given object, which is
/// normally either a `Future` or `Stream`.
///
/// This function is similar to the `thread::spawn` function but does not
/// attempt to run code in the background. The future will not make progress
/// until the methods on `Spawn` are called in turn.
pub fn spawn<T>(obj: T) -> Spawn<T> {
    Spawn {
        obj: obj,
        data: data::local_map(),
    }
}

impl<T> Spawn<T> {
    /// Get a shared reference to the object the Spawn is wrapping.
    pub fn get_ref(&self) -> &T {
        &self.obj
    }

    /// Get a mutable reference to the object the Spawn is wrapping.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.obj
    }

    /// Consume the Spawn, returning its inner object
    pub fn into_inner(self) -> T {
        self.obj
    }
}

impl<F: Future> Spawn<F> {
    /// Polls the internal future, scheduling notifications to be sent to the
    /// `unpark` argument.
    ///
    /// This method will poll the internal future, testing if it's completed
    /// yet. The `unpark` argument is used as a sink for notifications sent to
    /// this future. That is, while the future is being polled, any call to
    /// `task::park()` will return a handle that contains the `unpark`
    /// specified.
    ///
    /// If this function returns `NotReady`, then the `unpark` should have been
    /// scheduled to receive a notification when poll can be called again.
    /// Otherwise if `Ready` or `Err` is returned, the `Spawn` task can be
    /// safely destroyed.
    pub fn poll_future(&mut self, unpark: &Arc<Unpark>, unpark_id: u64)
        -> Poll<F::Item, F::Error>
    {
        self.enter(unpark, unpark_id, |f| f.poll())
    }

    /// Waits for the internal future to complete, blocking this thread's
    /// execution until it does.
    ///
    /// This function will call `poll_future` in a loop, waiting for the future
    /// to complete. When a future cannot make progress it will use
    /// `thread::park` to block the current thread.
    pub fn wait_future(&mut self) -> Result<F::Item, F::Error> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        let unpark2: Arc<Unpark> = unpark.clone();

        loop {
            match try!(self.poll_future(&unpark2, 0)) {
                Async::NotReady => unpark.park(),
                Async::Ready(e) => return Ok(e),
            }
        }
    }
}

impl<S: Stream> Spawn<S> {
    /// Like `poll_future`, except polls the underlying stream.
    pub fn poll_stream(&mut self, unpark: &Arc<Unpark>, unpark_id: u64)
        -> Poll<Option<S::Item>, S::Error>
    {
        self.enter(unpark, unpark_id, |stream| stream.poll())
    }

    /// Like `wait_future`, except only waits for the next element to arrive on
    /// the underlying stream.
    pub fn wait_stream(&mut self) -> Option<Result<S::Item, S::Error>> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        let unpark2: Arc<Unpark> = unpark.clone();

        loop {
            match self.poll_stream(&unpark2, 0) {
                Ok(Async::NotReady) => unpark.park(),
                Ok(Async::Ready(Some(e))) => return Some(Ok(e)),
                Ok(Async::Ready(None)) => return None,
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

impl<S: Sink> Spawn<S> {
    /// Invokes the underlying `start_send` method with this task in place.
    ///
    /// If the underlying operation returns `NotReady` then the `unpark` value
    /// passed in will receive a notification when the operation is ready to be
    /// attempted again.
    pub fn start_send(&mut self, value: S::SinkItem, unpark: &Arc<Unpark>, unpark_id: u64)
        -> StartSend<S::SinkItem, S::SinkError>
    {
        self.enter(unpark, unpark_id, |sink| sink.start_send(value))
    }

    /// Invokes the underlying `poll_complete` method with this task in place.
    ///
    /// If the underlying operation returns `NotReady` then the `unpark` value
    /// passed in will receive a notification when the operation is ready to be
    /// attempted again.
    pub fn poll_flush(&mut self, unpark: &Arc<Unpark>, unpark_id: u64)
        -> Poll<(), S::SinkError>
    {
        self.enter(unpark, unpark_id, |sink| sink.poll_complete())
    }

    /// Blocks the current thread until it's able to send `value` on this sink.
    ///
    /// This function will send the `value` on the sink that this task wraps. If
    /// the sink is not ready to send the value yet then the current thread will
    /// be blocked until it's able to send the value.
    pub fn wait_send(&mut self, mut value: S::SinkItem)
                     -> Result<(), S::SinkError> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        let unpark2 = unpark.clone() as Arc<Unpark>;
        loop {
            value = match try!(self.start_send(value, &unpark2, 0)) {
                AsyncSink::NotReady(v) => v,
                AsyncSink::Ready => return Ok(()),
            };
            unpark.park();
        }
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
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        let unpark2 = unpark.clone() as Arc<Unpark>;
        loop {
            if try!(self.poll_flush(&unpark2, 0)).is_ready() {
                return Ok(())
            }
            unpark.park();
        }
    }
}

impl<T> Spawn<T> {
    fn enter<F, R>(&mut self, unpark: &Arc<Unpark>, unpark_id: u64, f: F) -> R
        where F: FnOnce(&mut T) -> R
    {
        let borrowed = BorrowedTask {
            unpark: unpark,
            unpark_id: unpark_id,
            map: &self.data,
        };
        let obj = &mut self.obj;
        set(&borrowed, || f(obj))
    }
}

impl<T: fmt::Debug> fmt::Debug for Spawn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Spawn")
         .field("obj", &self.obj)
         .finish()
    }
}

/// A trait which represents a sink of notifications that a future is ready to
/// make progress.
///
/// This trait is provided as an argument to the `Spawn::poll_future` and
/// `Spawn::poll_stream` functions. It's transitively used as part of the
/// `Task::unpark` method to internally deliver notifications of readiness of a
/// future to move forward.
pub trait Unpark: Send + Sync {
    /// Indicates that an associated future and/or task are ready to make
    /// progress.
    ///
    /// Typically this means that the receiver of the notification should
    /// arrange for the future to get poll'd in a prompt fashion.
    fn unpark(&self, id: u64);

    /// A new `Task` handle referencing `id` has been created.
    fn ref_inc(&self, id: u64) {}

    /// A `Task` handle referencing `id` has been dropped.
    fn ref_dec(&self, id: u64) {}

    /// This fn only exists to support the legacy task system. It should **not**
    /// be implemented and will go away in the near future
    #[deprecated(since = "0.1.12", note = "do not use")]
    #[doc(hidden)]
    fn is_current(&self) -> Option<bool> {
        None
    }
}

// ===== UnparkContext =====

/// An unpark context allows for more fine grained unpark events.
///
/// See `with` function documentation for more detail.
#[derive(Debug)]
pub struct UnparkContext<T> {
    inner: Arc<Inner<T>>,
}

impl<T: Unpark + 'static> UnparkContext<T> {
    /// Create a new `UnparkContext` backed by `unpark`.
    pub fn new(unpark: T) -> UnparkContext<T> {
        UnparkContext {
            inner: Arc::new(Inner {
                obj: unpark,
                parent: AtomicTask::new(),
            }),
        }
    }

    /// Gets a reference to the underlying `Unpark`
    pub fn get_ref(&self) -> &T {
        &self.inner.obj
    }

    /// Execute the closure within the given unpark context.
    ///
    /// Unpark contexts are used to pass information about what event caused a
    /// task to be unparked. In some cases, tasks are waiting on a large number
    /// of possible events and need precise information about the wakeup to
    /// avoid extraneous polling.
    ///
    /// For the duration of the given function, all calls to `task::park()` will
    /// return a `Task` handle representing this unpark context. The unpark
    /// context represents the `Unpark` handle stored by the current context as
    /// well as the parent unpark context. This ensures that unpark events are
    /// correctly dispatched up the stack.
    ///
    /// This function must be called from within the context of an already
    /// running task.
    pub fn with<F, R>(&mut self, unpark_id: u64, f: F) -> R
        where F: FnOnce() -> R
    {
        let unpark = self.inner.clone() as Arc<Unpark>;

        unsafe {
            // `park` on atomic task requires a guarantee of mutal exclusion.
            // This is enforced by having `&mut self` on `with`.
            self.inner.parent.park();
        }

        with(|task| {
            let new_task = BorrowedTask {
                unpark: &unpark,
                unpark_id: unpark_id,
                map: task.map,
            };

            set(&new_task, f)
        })
    }
}

#[derive(Debug)]
struct Inner<T> {
    obj: T,
    parent: AtomicTask,
}

impl<T: Unpark> Unpark for Inner<T> {
    fn unpark(&self, unpark_id: u64) {
        self.obj.unpark(unpark_id);
        self.parent.unpark();
    }
}

// ===== ThreadUnpark =====

struct ThreadUnpark {
    thread: thread::Thread,
    ready: AtomicBool,
}

impl ThreadUnpark {
    fn new(thread: thread::Thread) -> ThreadUnpark {
        ThreadUnpark {
            thread: thread,
            ready: AtomicBool::new(false),
        }
    }

    fn park(&self) {
        if !self.ready.swap(false, Ordering::SeqCst) {
            thread::park();
        }
    }
}

impl Unpark for ThreadUnpark {
    fn unpark(&self, unpark_id: u64) {
        self.ready.store(true, Ordering::SeqCst);
        self.thread.unpark()
    }
}

// ===== Legacy task system =====

/// For the duration of the given callback, add an "unpark event" to be
/// triggered when the task handle is used to unpark the task.
///
/// Unpark events are used to pass information about what event caused a task to
/// be unparked. In some cases, tasks are waiting on a large number of possible
/// events, and need precise information about the wakeup to avoid extraneous
/// polling.
///
/// Every `Task` handle comes with a set of unpark events which will fire when
/// `unpark` is called. When fired, these events insert an identifer into a
/// concurrent set, which the task can read from to determine what events
/// occurred.
///
/// This function immediately invokes the closure, `f`, but arranges things so
/// that `task::park` will produce a `Task` handle that includes the given
/// unpark event.
///
/// # Panics
///
/// This function will panic if a task is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
pub fn with_unpark_event<F, R>(event: UnparkEvent, f: F) -> R
    where F: FnOnce() -> R
{
    struct ChainedUnpark {
        parent: Arc<Unpark>,
        parent_id: u64,
        event: UnparkEvent,
    }

    impl Unpark for ChainedUnpark {
        fn unpark(&self, unpark_id: u64) {
            self.event.unpark();
            self.parent.unpark(self.parent_id);
        }

        fn is_current(&self) -> Option<bool> {
            self.parent.is_current()
        }
    }

    let unpark2 = with(|t| {
        Arc::new(ChainedUnpark {
            parent: t.unpark.clone(),
            parent_id: t.unpark_id,
            event: event,
        })
    });

    let unpark_id = unpark2.parent_id;

    let mut s = spawn(());
    let unpark2: Arc<Unpark> = unpark2;

    s.enter(&unpark2, unpark_id, |_| f())
}

#[derive(Clone)]
/// A set insertion to trigger upon `unpark`.
///
/// Unpark events are used to communicate information about *why* an unpark
/// occured, in particular populating sets with event identifiers so that the
/// unparked task can avoid extraneous polling. See `with_unpark_event` for
/// more.
pub struct UnparkEvent {
    set: Arc<EventSet>,
    item: usize,
}

impl UnparkEvent {
    /// Construct an unpark event that will insert `id` into `set` when
    /// triggered.
    pub fn new(set: Arc<EventSet>, id: usize) -> UnparkEvent {
        UnparkEvent {
            set: set,
            item: id,
        }
    }

    fn unpark(&self) {
        self.set.insert(self.item);
    }
}

impl fmt::Debug for UnparkEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UnparkEvent")
         .field("set", &"...")
         .field("item", &self.item)
         .finish()
    }
}

/// A concurrent set which allows for the insertion of `usize` values.
///
/// `EventSet`s are used to communicate precise information about the event(s)
/// that trigged a task notification. See `task::with_unpark_event` for details.
pub trait EventSet: Send + Sync + 'static {
    /// Insert the given ID into the set
    fn insert(&self, id: usize);
}
