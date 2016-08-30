//! Tasks used to drive a future computation
//!
//! It's intended over time a particular operation (such as servicing an HTTP
//! request) will involve many futures. This entire operation, however, can be
//! thought of as one unit, as the entire result is essentially just moving
//! through one large state machine.
//!
//! A "task" is the unit of abstraction for what is driving this state machine
//! and tree of futures forward. A task is used to poll futures and schedule
//! futures with, and has utilities for sharing data between tasks and handles
//! for notifying when a future is ready. Each task also has its own set of
//! task-local data, which can be accessed at any point by the task's future;
//! see `with_local_data`.
//!
//! Note that libraries typically should not manage tasks themselves, but rather
//! leave that to event loops and other "executors", or by using the `wait`
//! method to create and execute a task directly on the current thread.
//!
//! There are two basic execution models for tasks: via an `Executor` (which is
//! generally one or more threads together with a queue of tasks to execute) or
//! by blocking on the current thread (via `ThreadTask`).
//!
//! ## Functions
//!
//! There is an important bare function in this module: `park`. The `park`
//! function is similar to the standard library's `thread::park` method where it
//! returns a handle to wake up a task at a later date (via an `unpark` method).

mod unpark_mutex;

use {BoxFuture, Poll};

use std::prelude::v1::*;

use std::thread;
use std::cell::{Cell, RefCell};
use std::sync::Arc;

use self::unpark_mutex::UnparkMutex;

use typemap::{SendMap, TypeMap};

thread_local!(static CURRENT_TASK: Cell<*const Task> = Cell::new(0 as *const _));
thread_local!(static CURRENT_TASK_DATA: Cell<*const TaskData> = Cell::new(0 as *const _));

fn set<F, R>(task: &Task, data: &TaskData, f: F) -> R
    where F: FnOnce() -> R
{
    struct Reset(*const Task, *const TaskData);
    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT_TASK.with(|c| c.set(self.0));
            CURRENT_TASK_DATA.with(|c| c.set(self.1));
        }
    }

    CURRENT_TASK.with(|c| {
        CURRENT_TASK_DATA.with(|d| {
            let _reset = Reset(c.get(), d.get());
            c.set(task);
            d.set(data);
            f()
        })
    })
}

fn with<F: FnOnce(&Task, &TaskData) -> R, R>(f: F) -> R {
    let task = CURRENT_TASK.with(|c| c.get());
    let data = CURRENT_TASK_DATA.with(|d| d.get());
    assert!(task != 0 as *const _, "No `Task` is currently running");
    assert!(data != 0 as *const _, "No `TaskData` is available; is a `Task` runnin?");
    unsafe {
        f(&*task, &*data)
    }
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
/// the `Task::with_unpark_event` for details on how to do this.
///
/// # Panics
///
/// This function will panic if a task is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
pub fn park() -> Task {
    with(|task, _| task.clone())
}

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
    with(|task, data| {
        let new_task = Task {
            kind: task.kind.clone(),
            events: task.events.with_event(event),
        };
        set(&new_task, data, f)
    })
}

/// Access the current task's local data.
///
/// Each task has its own set of local data, which is required to be `Send` but
/// not `Sync`. Futures within a task can access this data at any point.
///
/// # Panics
///
/// This function will panic if a task is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
///
/// It will also panic if called in the context of another `with_local_data` or
/// `with_local_data_mut`.
pub fn with_local_data<F, R>(f: F) -> R
    where F: FnOnce(&SendMap) -> R
{
    with(|_, data| {
        f(&*data.borrow())
    })
}

/// Access the current task's local data, mutably.
///
/// Each task has its own set of local data, which is required to be `Send` but
/// not `Sync`. Futures within a task can access this data at any point.
///
/// # Panics
///
/// This function will panic if a task is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
///
/// It will also panic if called in the context of another `with_local_data` or
/// `with_local_data_mut`.
pub fn with_local_data_mut<F, R>(f: F) -> R
    where F: FnOnce(&mut SendMap) -> R
{
    with(|_, data| {
        f(&mut *data.borrow_mut())
    })
}


/// A handle to a "task", which represents a single lightweight "thread" of
/// execution driving a future to completion.
///
/// In general, futures are composed into large units of work, which are then
/// spawned as tasks onto an *executor*. The executor is responible for polling
/// the future as notifications arrive, until the future terminates.
///
/// Obtained by the `task::park` function, or by binding to an executor through
/// the `Task::new` constructor.
#[derive(Clone)]
pub struct Task {
    kind: TaskKind,
    events: Events,
}

#[derive(Clone)]
/// The two kinds of execution models: `Executor`-based or local thread-based
enum TaskKind {
    Executor {
        mutex: Arc<UnparkMutex<MutexInner>>,
        exec: Arc<Executor>,
    },
    Local(thread::Thread),
}

type TaskData = RefCell<SendMap>;

struct MutexInner {
    future: BoxFuture<(), ()>,
    task_data: TaskData,
}

/// A task intended to be run directly on a local thread.
///
/// Actual execution of a `ThreadTask` is left to the thread itself, which can
/// use the `ThreadTask::enter` method to set up the environment for the task
/// before polling it.
///
/// When the corresponding `Task` handle is unparked, it will invoke
/// `std::thread::Thread::unpark` for the thread that entered the task.
pub struct ThreadTask {
    task_data: TaskData,
}

/// A handle for running an executor-bound task.
pub struct Run {
    task: Task,
    inner: MutexInner,
}

impl ThreadTask {
    /// Create a new `ThreadTask`; note that the task is in no way bound to the
    /// current thread.
    pub fn new() -> ThreadTask {
        ThreadTask {
            task_data: RefCell::new(TypeMap::custom()),
        }
    }

    /// "Enter" the task, setting up the task environment appropriately for
    /// calling `poll` methods.
    ///
    /// Ties the ambient `Task` handle to `std::thread::Thread::unpark` for the
    /// current thread.
    pub fn enter<R, F: FnOnce() -> R>(&self, f: F) -> R {
        let task = Task {
            kind: TaskKind::Local(thread::current()),
            events: Events::new(),
        };
        set(&task, &self.task_data, f)
    }
}


impl Run {
    /// Actually run the task (invoking `poll` on its future) on the current
    /// thread.
    pub fn run(self) {
        // TODO: make this more efficient (avoid clone, etc)
        let mutex = match self.task.kind {
            TaskKind::Executor { ref mutex, .. } => mutex.clone(),
            _ => unreachable!(),
        };

        let Run { task, inner: MutexInner { mut future, mut task_data } } = self;

        loop {
            unsafe {
                // SAFETY: the ownership of this `Run` object is evidence that
                // we are in the `POLLING`/`REPOLL` state for the mutex.
                mutex.start_poll();
            }

            if let Poll::NotReady = set(&task, &task_data, || future.poll()) {
                let wait_res = unsafe {
                    // SAFETY: same as above
                    mutex.wait(MutexInner {
                        future: future,
                        task_data: task_data
                    })
                };
                if let Err(new_inner) = wait_res {
                    future = new_inner.future;
                    task_data = new_inner.task_data;
                } else {
                    return;
                }
            } else {
                unsafe {
                    // SAFETY: same as above
                    mutex.complete();
                }
                return;
            }
        }
    }
}

// A collection of UnparkEvents to trigger on `unpark`
#[derive(Clone)]
struct Events {
    set: Vec<UnparkEvent>, // TODO: change to some SmallVec
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

/// A way of notifying a task that it should wake up and poll its future.
pub trait Executor: Send + Sync + 'static {
    /// Indicate that the task should attempt to poll its future in a timely
    /// fashion. This is typically done when alerting a future that an event of
    /// interest has occurred through `Task::unpark`.
    ///
    /// It must be guaranteed that, for each call to `notify`, `poll` will be
    /// called at least once subsequently (unless the task has terminated). If
    /// the task is currently polling its future when `notify` is called, it
    /// must poll the future *again* afterwards, ensuring that all relevant
    /// events are eventually observed by the future.
    fn execute(&self, run: Run);
}

/// A concurrent set which allows for the insertion of `usize` values.
///
/// `EventSet`s are used to communicate precise information about the event(s)
/// that trigged a task notification. See `task::with_unpark_event` for details.
pub trait EventSet: Send + Sync + 'static {
    /// Insert the given ID into the set
    fn insert(&self, id: usize);
}

impl Task {
    /// Creates a new task by binding together a future and an executor.
    ///
    /// Does not actually begin task execution; use the `unpark` method to do
    /// so.
    pub fn new(exec: Arc<Executor>, future: BoxFuture<(), ()>) -> Task {
        Task {
            kind: TaskKind::Executor {
                mutex: Arc::new(UnparkMutex::new(MutexInner {
                    future: future,
                    task_data: RefCell::new(TypeMap::custom()),
                })),
                exec: exec
            },
            events: Events::new(),
        }
    }

    /// Indicate that the task should attempt to poll its future in a timely
    /// fashion. This is typically done when alerting a future that an event of
    /// interest has occurred through `Task::unpark`.
    ///
    /// It's guaranteed that, for each call to `notify`, `poll` will be called
    /// at least once subsequently (unless the task has terminated). If the task
    /// is currently polling its future when `notify` is called, it must poll
    /// the future *again* afterwards, ensuring that all relevant events are
    /// eventually observed by the future.
    pub fn unpark(&self) {
        self.events.trigger();

        match self.kind {
            TaskKind::Executor { ref mutex, ref exec } => {
                if let Ok(inner) = mutex.notify() {
                    let task = Task {
                        kind: self.kind.clone(),
                        events: Events::new(),
                    };
                    let run = Run {
                        task: task,
                        inner: inner,
                    };
                    exec.execute(run)
                }
            }
            TaskKind::Local(ref thread) => thread.unpark()
        }
    }

    /// Determines whether the underlying future has ever polled with a final
    /// result (or panicked), and thus terminated.
    pub fn is_done(&self) -> bool {
        match self.kind {
            TaskKind::Executor { ref mutex, .. } => mutex.is_complete(),
            _ => false,
        }
    }
}

impl Events {
    fn new() -> Events {
        Events { set: Vec::new() }
    }

    fn trigger(&self) {
        for event in &self.set {
            event.set.insert(event.item)
        }
    }

    fn with_event(&self, event: UnparkEvent) -> Events {
        let mut set = self.set.clone();
        set.push(event);
        Events{ set: set }
    }
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
}
