use std::prelude::v1::*;

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use {Poll, Future, Async, Stream, Sink, StartSend, AsyncSink};
use future::BoxFuture;

mod unpark_mutex;
use self::unpark_mutex::UnparkMutex;

mod data;
pub use self::data::LocalKey;

mod atomic_task;
pub use self::atomic_task::AtomicTask;

mod task_rc;
#[allow(deprecated)]
#[cfg(feature = "with-deprecated")]
pub use self::task_rc::TaskRc;

struct BorrowedTask<'a> {
    unpark: BorrowedUnpark<'a>,
    events: BorrowedEvents<'a>,
    // Task-local storage
    map: &'a data::LocalMap,
}

#[derive(Copy, Clone)]
#[allow(deprecated)]
enum BorrowedUnpark<'a> {
    Old(&'a Arc<Unpark>),
    New(&'a Fn() -> NotifyHandle, u64),
}

#[derive(Copy, Clone)]
#[allow(deprecated)]
enum BorrowedEvents<'a> {
    None,
    One(&'a UnparkEvent, &'a BorrowedEvents<'a>),
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
/// This is obtained by the `task::current` function.
pub struct Task {
    unpark: TaskUnpark,
    events: UnparkEvents,
}

fn _assert_kinds() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Task>();
}

enum TaskUnpark {
    #[allow(deprecated)]
    Old(Arc<Unpark>),
    New(NotifyHandle, u64),
}

#[derive(Clone)]
#[allow(deprecated)]
enum UnparkEvents {
    None,
    One(UnparkEvent),
    Many(Box<[UnparkEvent]>),
}

/// Returns a handle to the current task to call `notify` at a later date.
///
/// The returned handle implements the `Send` and `'static` bounds and may also
/// be cheaply cloned. This is useful for squirreling away the handle into a
/// location which is then later signaled that a future can make progress.
///
/// Implementations of the `Future` trait typically use this function if they
/// would otherwise perform a blocking operation. When something isn't ready
/// yet, this `current` function is called to acquire a handle to the current
/// task, and then the future arranges it such that when the blocking operation
/// otherwise finishes (perhaps in the background) it will `notify` the
/// returned handle.
///
/// It's sometimes necessary to pass extra information to the task when
/// unparking it, so that the task knows something about *why* it was woken.
/// See the `FutureQueue` documentation for details on how to do this.
///
/// # Panics
///
/// This function will panic if a task is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
pub fn current() -> Task {
    with(|borrowed| {
        let unpark = match borrowed.unpark {
            BorrowedUnpark::Old(old) => TaskUnpark::Old(old.clone()),
            BorrowedUnpark::New(new, id) => {
                // A new handle is being created, increment the ref count
                let handle = new();
                handle.ref_inc(id);
                TaskUnpark::New(handle, id)
            }
        };

        let mut one_event = None;
        let mut list = Vec::new();
        let mut cur = &borrowed.events;
        while let BorrowedEvents::One(event, next) = *cur {
            let event = event.clone();
            match one_event.take() {
                None if list.len() == 0 => one_event = Some(event),
                None => list.push(event),
                Some(event2) =>  {
                    list.push(event2);
                    list.push(event);
                }
            }
            cur = next;
        }

        let events = match one_event {
            None if list.len() == 0 => UnparkEvents::None,
            None => UnparkEvents::Many(list.into_boxed_slice()),
            Some(e) => UnparkEvents::One(e),
        };

        Task {
            unpark: unpark,
            events: events,
        }
    })
}

#[doc(hidden)]
#[deprecated(note = "renamed to `current`")]
pub fn park() -> Task {
    current()
}

impl Task {
    /// Indicate that the task should attempt to poll its future in a timely
    /// fashion.
    ///
    /// It's typically guaranteed that, for each call to `notify`, `poll` will
    /// be called at least once subsequently (unless the future has terminated).
    /// If the task is currently polling its future when `notify` is called, it
    /// must poll the future *again* afterwards, ensuring that all relevant
    /// events are eventually observed by the future.
    #[allow(deprecated)]
    pub fn notify(&self) {
        match self.events {
            UnparkEvents::None => {}
            UnparkEvents::One(ref e) => e.unpark(),
            UnparkEvents::Many(ref list) => {
                for event in list.iter() {
                    event.unpark();
                }
            }
        }

        match self.unpark {
            TaskUnpark::Old(ref old) => old.unpark(),
            TaskUnpark::New(ref new, id) => new.notify(id),
        }
    }

    #[doc(hidden)]
    #[deprecated(note = "renamed to `notify`")]
    pub fn unpark(&self) {
        self.notify()
    }

    /// Returns `true` when called from within the context of the task. In
    /// other words, the task is currently running on the thread calling the
    /// function.
    pub fn is_current(&self) -> bool {
        panic!()
        // with(|current| {
        //     if current.unpark_id != self.unpark_id {
        //         return false;
        //     }
        //
        //     if let Some(eq) = self.unpark.is_current() {
        //         // Handles legacy task system...
        //         eq
        //     } else {
        //         let a = &**current.unpark as *const Unpark;
        //         let b = &*self.unpark as *const Unpark;
        //
        //         if a != b {
        //             return false;
        //         }
        //
        //         true
        //     }
        // })
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Task")
         .finish()
    }
}

impl Clone for Task {
    fn clone(&self) -> Task {
        let unpark = match self.unpark {
            TaskUnpark::Old(ref old) => TaskUnpark::Old(old.clone()),
            TaskUnpark::New(ref new, id) => {
                new.ref_inc(id);
                TaskUnpark::New(new.clone(), id)
            }
        };

        Task {
            unpark: unpark,
            events: self.events.clone(),
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        if let TaskUnpark::New(ref new, id) = self.unpark {
            new.ref_dec(id);
        }
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
    #[deprecated(note = "recommended to use `poll_future_notify` instead")]
    #[allow(deprecated)]
    pub fn poll_future(&mut self, unpark: Arc<Unpark>) -> Poll<F::Item, F::Error> {
        self.enter(BorrowedUnpark::Old(&unpark), |f| f.poll())
    }

    /// Polls the internal future, scheduling notifications to be sent to the
    /// `notify` argument.
    ///
    /// This method will poll the internal future, testing if it's completed
    /// yet. The `notify` argument is used as a sink for notifications sent to
    /// this future. That is, while the future is being polled, any call to
    /// `task::current()` will return a handle that contains the `notify`
    /// specified.
    ///
    /// If this function returns `NotReady`, then the `notify` should have been
    /// scheduled to receive a notification when poll can be called again.
    /// Otherwise if `Ready` or `Err` is returned, the `Spawn` task can be
    /// safely destroyed.
    ///
    /// Note that `notify` itself is passed as a shared reference, and is itself
    /// not required to be a `NotifyHandle`. The `Clone` and `Into` trait bounds
    /// will be used to convert this `notify` to a `NotifyHandle` if necessary.
    /// This construction can avoid an unnecessary atomic reference count bump
    /// in some situations.
    pub fn poll_future_notify<T>(&mut self,
                                 notify: &T,
                                 id: u64) -> Poll<F::Item, F::Error>
        where T: Clone + Into<NotifyHandle>,
    {
        let mk = || notify.clone().into();
        self.enter(BorrowedUnpark::New(&mk, id), |f| f.poll())
    }

    /// Waits for the internal future to complete, blocking this thread's
    /// execution until it does.
    ///
    /// This function will call `poll_future` in a loop, waiting for the future
    /// to complete. When a future cannot make progress it will use
    /// `thread::park` to block the current thread.
    pub fn wait_future(&mut self) -> Result<F::Item, F::Error> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));

        loop {
            match try!(self.poll_future_notify(&unpark, 0)) {
                Async::NotReady => unpark.park(),
                Async::Ready(e) => return Ok(e),
            }
        }
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
    pub fn execute(self, exec: Arc<Executor>)
        where F: Future<Item=(), Error=()> + Send + 'static,
    {
        exec.clone().execute(Run {
            // Ideally this method would be defined directly on
            // `Spawn<BoxFuture<(), ()>>` so we wouldn't have to box here and
            // it'd be more explicit, but unfortunately that currently has a
            // link error on nightly: rust-lang/rust#36155
            spawn: spawn(self.into_inner().boxed()),
            inner: Arc::new(RunInner {
                exec: exec,
                mutex: UnparkMutex::new()
            }),
        })
    }
}

impl<S: Stream> Spawn<S> {
    /// Like `poll_future`, except polls the underlying stream.
    #[deprecated(note = "recommended to use `poll_stream_notify` instead")]
    #[allow(deprecated)]
    pub fn poll_stream(&mut self, unpark: Arc<Unpark>)
                       -> Poll<Option<S::Item>, S::Error> {
        self.enter(BorrowedUnpark::Old(&unpark), |s| s.poll())
    }

    /// Like `poll_future_notify`, except polls the underlying stream.
    pub fn poll_stream_notify<T>(&mut self,
                                 notify: &T,
                                 id: u64)
                                 -> Poll<Option<S::Item>, S::Error>
        where T: Clone + Into<NotifyHandle>,
    {
        let mk = || notify.clone().into();
        self.enter(BorrowedUnpark::New(&mk, id), |s| s.poll())
    }

    /// Like `wait_future`, except only waits for the next element to arrive on
    /// the underlying stream.
    pub fn wait_stream(&mut self) -> Option<Result<S::Item, S::Error>> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        loop {
            match self.poll_stream_notify(&unpark, 0) {
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
    #[deprecated(note = "recommended to use `start_send_notify` instead")]
    #[allow(deprecated)]
    pub fn start_send(&mut self, value: S::SinkItem, unpark: &Arc<Unpark>)
                       -> StartSend<S::SinkItem, S::SinkError> {
        self.enter(BorrowedUnpark::Old(unpark), |s| s.start_send(value))
    }

    /// Invokes the underlying `start_send` method with this task in place.
    ///
    /// If the underlying operation returns `NotReady` then the `notify` value
    /// passed in will receive a notification when the operation is ready to be
    /// attempted again.
    pub fn start_send_notify<T>(&mut self,
                                value: S::SinkItem,
                                notify: &T,
                                id: u64)
                               -> StartSend<S::SinkItem, S::SinkError>
        where T: Clone + Into<NotifyHandle>,
    {
        let mk = || notify.clone().into();
        self.enter(BorrowedUnpark::New(&mk, id), |s| s.start_send(value))
    }

    /// Invokes the underlying `poll_complete` method with this task in place.
    ///
    /// If the underlying operation returns `NotReady` then the `unpark` value
    /// passed in will receive a notification when the operation is ready to be
    /// attempted again.
    #[deprecated(note = "recommended to use `poll_flush_notify` instead")]
    #[allow(deprecated)]
    pub fn poll_flush(&mut self, unpark: &Arc<Unpark>)
                       -> Poll<(), S::SinkError> {
        self.enter(BorrowedUnpark::Old(unpark), |s| s.poll_complete())
    }

    /// Invokes the underlying `poll_complete` method with this task in place.
    ///
    /// If the underlying operation returns `NotReady` then the `notify` value
    /// passed in will receive a notification when the operation is ready to be
    /// attempted again.
    pub fn poll_flush_notify<T>(&mut self,
                                notify: &T,
                                id: u64)
                                -> Poll<(), S::SinkError>
        where T: Clone + Into<NotifyHandle>,
    {
        let mk = || notify.clone().into();
        self.enter(BorrowedUnpark::New(&mk, id), |s| s.poll_complete())
    }

    /// Blocks the current thread until it's able to send `value` on this sink.
    ///
    /// This function will send the `value` on the sink that this task wraps. If
    /// the sink is not ready to send the value yet then the current thread will
    /// be blocked until it's able to send the value.
    pub fn wait_send(&mut self, mut value: S::SinkItem)
                     -> Result<(), S::SinkError> {
        let notify = Arc::new(ThreadUnpark::new(thread::current()));
        loop {
            value = match try!(self.start_send_notify(value, &notify, 0)) {
                AsyncSink::NotReady(v) => v,
                AsyncSink::Ready => return Ok(()),
            };
            notify.park();
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
        let notify = Arc::new(ThreadUnpark::new(thread::current()));
        loop {
            if try!(self.poll_flush_notify(&notify, 0)).is_ready() {
                return Ok(())
            }
            notify.park();
        }
    }
}

impl<T> Spawn<T> {
    fn enter<F, R>(&mut self, unpark: BorrowedUnpark, f: F) -> R
        where F: FnOnce(&mut T) -> R
    {
        let borrowed = BorrowedTask {
            unpark: unpark,
            events: BorrowedEvents::None,
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
#[deprecated(note = "recommended to use `Notify` instead")]
pub trait Unpark: Send + Sync {
    /// Indicates that an associated future and/or task are ready to make
    /// progress.
    ///
    /// Typically this means that the receiver of the notification should
    /// arrange for the future to get poll'd in a prompt fashion.
    fn unpark(&self);
}

/// A trait representing requests to poll futures.
///
/// This trait is an argument to the `Spawn::execute` which is used to run a
/// future to completion. An executor will receive requests to run a future and
/// an executor is responsible for ensuring that happens in a timely fashion.
pub trait Executor: Send + Sync + 'static {
    /// Requests that `Run` is executed soon on the given executor.
    fn execute(&self, r: Run);
}

/// Units of work submitted to an `Executor`, currently only created
/// internally.
pub struct Run {
    spawn: Spawn<BoxFuture<(), ()>>,
    inner: Arc<RunInner>,
}

struct RunInner {
    mutex: UnparkMutex<Run>,
    exec: Arc<Executor>,
}

impl Run {
    /// Actually run the task (invoking `poll` on its future) on the current
    /// thread.
    #[allow(deprecated)]
    pub fn run(self) {
        let Run { mut spawn, inner } = self;

        // SAFETY: the ownership of this `Run` object is evidence that
        // we are in the `POLLING`/`REPOLL` state for the mutex.
        unsafe {
            inner.mutex.start_poll();

            loop {
                let unpark: Arc<Unpark> = inner.clone();
                match spawn.poll_future(unpark) {
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

#[allow(deprecated)]
impl Unpark for RunInner {
    fn unpark(&self) {
        match self.mutex.notify() {
            Ok(run) => self.exec.execute(run),
            Err(()) => {}
        }
    }
}

/// A trait which represents a sink of notifications that a future is ready to
/// make progress.
///
/// This trait is provided as an argument to the `Spawn::*_notify` family of
/// functions. It's transitively used as part of the `Task::notify` method to
/// internally deliver notifications of readiness of a future to move forward.
///
/// An instance of `Notify` has one primary method, `notify`, which is given a
/// contextual argument as to what's being notified. This contextual argument is
/// *also* provided to the `Spawn::*_notify` family of functions and can be used
/// to reuse an instance of `Notify` across many futures.
///
/// Instances of `Notify` must be safe to share across threads, and the methods
/// be invoked concurrently. They must also live for the `'static` lifetime,
/// not containing any stack references.
pub trait Notify: Send + Sync {
    /// Indicates that an associated future and/or task are ready to make
    /// progress.
    ///
    /// Typically this means that the receiver of the notification should
    /// arrange for the future to get poll'd in a prompt fashion.
    ///
    /// This method takes an `id` as an argument which was transitively passed
    /// in from the original call to `Spawn::*_notify`. This id can be used to
    /// disambiguate which precise future became ready for polling.
    fn notify(&self, id: u64);

    /// A new `Task` handle referencing `id` has been created.
    #[allow(unused_variables)]
    fn ref_inc(&self, id: u64) {}

    /// A `Task` handle referencing `id` has been dropped.
    #[allow(unused_variables)]
    fn ref_dec(&self, id: u64) {}

    /// This fn only exists to support the legacy task system. It should **not**
    /// be implemented and will go away in the near future
    #[deprecated(since = "0.1.12", note = "do not use")]
    #[doc(hidden)]
    fn is_current(&self) -> Option<bool> {
        None
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

impl Notify for ThreadUnpark {
    fn notify(&self, _unpark_id: u64) {
        self.ready.store(true, Ordering::SeqCst);
        self.thread.unpark()
    }
}

// ===== UnparkEvent =====

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
#[deprecated(note = "recommended to use `FuturesUnordered` instead")]
#[allow(deprecated)]
pub fn with_unpark_event<F, R>(event: UnparkEvent, f: F) -> R
    where F: FnOnce() -> R
{
    with(|task| {
        let new_task = BorrowedTask {
            unpark: task.unpark,
            events: BorrowedEvents::One(&event, &task.events),
            map: task.map,
        };

        set(&new_task, f)
    })
}

pub fn with_notify<F, T, R>(notify: &T, id: u64, f: F) -> R
    where F: FnOnce() -> R,
          T: Clone + Into<NotifyHandle>,
{
    with(|task| {
        let mk = || notify.clone().into();
        let new_task = BorrowedTask {
            unpark: BorrowedUnpark::New(&mk, id),
            events: task.events,
            map: task.map,
        };

        set(&new_task, f)
    })
}

/// A set insertion to trigger upon `unpark`.
///
/// Unpark events are used to communicate information about *why* an unpark
/// occured, in particular populating sets with event identifiers so that the
/// unparked task can avoid extraneous polling. See `with_unpark_event` for
/// more.
#[derive(Clone)]
#[deprecated(note = "recommended to use `FuturesUnordered` instead")]
#[allow(deprecated)]
pub struct UnparkEvent {
    set: Arc<EventSet>,
    item: usize,
}

#[allow(deprecated)]
impl UnparkEvent {
    /// Construct an unpark event that will insert `id` into `set` when
    /// triggered.
    #[deprecated(note = "recommended to use `FuturesUnordered` instead")]
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

#[allow(deprecated)]
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

/// An unsafe trait for implementing custom forms of memory management behind a
/// `Task`.
///
/// The `futures` critically relies on "notification handles" to extract for
/// futures to contain and then later inform that they're ready to make
/// progress. These handles, however, must be cheap to create and cheap
/// to clone to ensure that this operation is efficient throughout the
/// execution of a program.
///
/// Typically this sort of memory management is done in the standard library
/// with the `Arc` type. An `Arc` is relatively cheap to allocate an is
/// quite cheap to clone and pass around. Plus, it's 100% safe!
///
/// When working outside the standard library, however, you don't always have
/// and `Arc` type available to you. This trait, `UnsafeNotify`, is intended
/// to be the "unsafe version" of the `Notify` trait. This trait encodes the
/// memory management operations of a `Task`'s notification handle, allowing
/// custom implementations for the memory management of a notification handle.
///
/// Put another way, the core notification type in this library,
/// `NotifyHandle`, simply internally contains an instance of
/// `*mut UnsafeNotify`. This "unsafe trait object" is then used exclusively
/// to operate with, dynamically dispatching calls to clone, drop, and notify.
/// Critically though as a raw pointer it doesn't require a particular form
/// of memory management, allowing external implementations.
///
/// A default implementation of the `UnsafeNotify` trait is provided for the
/// `Arc` type in the standard library. If the `use_std` feature of this crate
/// is not available however, you'll be required to implement your own
/// instance of this trait to pass it into `NotifyHandle::new`.
///
/// # Unsafety
///
/// This trait is manually encoding the memory management of the underlying
/// handle, and as a result is quite unsafe to implement! Implementors of
/// this trait must guarantee:
///
/// * Calls to `clone_raw` produce uniquely owned handles. It should be safe
///   to drop the current handle and have the returned handle still be valid.
/// * Calls to `drop_raw` work with `self` as a raw pointer, deallocating
///   resources associated with it. This is a pretty unsafe operation as it's
///   invalidating the `self` pointer, so extreme care needs to be taken.
///
/// In general it's recommended to review the trait documentation as well as
/// the implementation for `Arc` in this crate. When in doubt ping the
/// `futures` authors to clarify an unsafety question here.
pub unsafe trait UnsafeNotify: Notify {
    /// Creates a new `NotifyHandle` from this instance of `UnsafeNotify`.
    ///
    /// This function will create a new uniquely owned handle that under the
    /// hood references the same notification instance. In other words calls
    /// to `notify` on the returned handle should be equivalent to calls to
    /// `notify` on this handle.
    ///
    /// # Unsafety
    ///
    /// This trait is unsafe to implement, as are all these methods. This
    /// method is also unsafe to call as it's asserting the `UnsafeNotify`
    /// value is in a consistent state. In general it's recommended to
    /// review the trait documentation as well as the implementation for `Arc`
    /// in this crate. When in doubt ping the `futures` authors to clarify
    /// an unsafety question here.
    unsafe fn clone_raw(&self) -> NotifyHandle;

    /// Drops this instance of `UnsafeNotify`, deallocating resources
    /// associated with it.
    ///
    /// This method is intended to have a signature such as:
    ///
    /// ```ignore
    /// fn drop_raw(self: *mut Self);
    /// ```
    ///
    /// Unfortunately in Rust today that signature is not object safe.
    /// Nevertheless it's recommended to implement this function *as if* that
    /// were its signature. As such it is not safe to call on an invalid
    /// pointer, nor is the validity of the pointer guaranteed after this
    /// function returns.
    ///
    /// # Unsafety
    ///
    /// This trait is unsafe to implement, as are all these methods. This
    /// method is also unsafe to call as it's asserting the `UnsafeNotify`
    /// value is in a consistent state. In general it's recommended to
    /// review the trait documentation as well as the implementation for `Arc`
    /// in this crate. When in doubt ping the `futures` authors to clarify
    /// an unsafety question here.
    unsafe fn drop_raw(&self);
}

/// A `NotifyHandle` is the core value through which notifications are routed
/// in the `futures` crate.
///
/// All instances of `Task` will contain a `NotifyHandle` handle internally.
/// This handle itself contains a trait object pointing to an instance of the
/// `Notify` trait, allowing notifications to get routed through it.
///
/// The `NotifyHandle` type internally does not codify any particular memory
/// management strategy. Internally it contains an instance of `*mut
/// UnsafeNotify`, and more details about that trait can be found on its own
/// documentation. Consequently, though, the one constructor of this type,
/// `NotifyHandle::new`, is `unsafe` to call. It is not recommended to call
/// this constructor directly.
///
/// If you're working with the standard library then it's recommended to
/// work with the `Arc` type. If you have a struct, `T`, which implements the
/// `Notify` trait, then you can construct this with
/// `NotifyHandle::from(t: Arc<T>)`. The coercion to `UnsafeNotify` will
/// happen automatically and safely for you.
///
/// When working externally from the standard library it's recommended to
/// provide a similar safe constructor for your custom type as opposed to
/// recommending an invocation of `NotifyHandle::new` directly.
pub struct NotifyHandle {
    inner: *mut UnsafeNotify,
}

unsafe impl Send for NotifyHandle {}
unsafe impl Sync for NotifyHandle {}

impl NotifyHandle {
    /// Constructs a new `NotifyHandle` directly.
    ///
    /// Note that most code will not need to call this. Implementors of the
    /// `UnsafeNotify` trait will typically provide a wrapper that calls this
    /// but you otherwise shouldn't call it directly.
    ///
    /// If you're working with the standard library then it's recommended to
    /// use the `NotifyHandle::from` function instead which works with the safe
    /// `Arc` type and the safe `Notify` trait.
    pub unsafe fn new(inner: *mut UnsafeNotify) -> NotifyHandle {
        NotifyHandle { inner: inner }
    }

    /// Invokes the underlying instance of `Notify` with the provided `id`.
    pub fn notify(&self, id: u64) {
        unsafe { (*self.inner).notify(id) }
    }

    fn ref_inc(&self, id: u64) {
        unsafe { (*self.inner).ref_inc(id) }
    }

    fn ref_dec(&self, id: u64) {
        unsafe { (*self.inner).ref_dec(id) }
    }
}

impl Clone for NotifyHandle {
    fn clone(&self) -> Self {
        unsafe {
            (*self.inner).clone_raw()
        }
    }
}

impl fmt::Debug for NotifyHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("NotifyHandle")
         .finish()
    }
}

impl Drop for NotifyHandle {
    fn drop(&mut self) {
        unsafe {
            (*self.inner).drop_raw()
        }
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
// This also means that you wn't actually fine `UnsafeNotify for Arc<T>`
// because it's the wrong level of indirection. These methods are sort of
// receiving Arc<T>, but not an owned version. It's... complicated. We may be
// one of the first users of unsafe trait objects!

struct ArcWrapped<T>(PhantomData<T>);

impl<T: Notify + 'static> Notify for ArcWrapped<T> {
    fn notify(&self, id: u64) {
        unsafe {
            let me: *const ArcWrapped<T> = self;
            T::notify(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>),
                      id)
        }
    }

    fn ref_inc(&self, id: u64) {
        unsafe {
            let me: *const ArcWrapped<T> = self;
            T::ref_inc(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>),
                      id)
        }
    }

    fn ref_dec(&self, id: u64) {
        unsafe {
            let me: *const ArcWrapped<T> = self;
            T::ref_dec(&*(&me as *const *const ArcWrapped<T> as *const Arc<T>),
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
