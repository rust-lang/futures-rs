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
//! for notifying when a future is ready.
//!
//! Note that libraries typically should not manage tasks themselves, but rather
//! leave that to event loops at the top level. The `spawn` function should be
//! used to "spawn" a future, or an event loop should be used to run a specific
//! future to completion.
//!
//! ## Functions
//!
//! There is an important bare function in this module: `park`. The `park`
//! function is similar to the standard library's `thread::park` method where it
//! returns a handle to wake up a task at a later date (via an `unpark` method).

use std::prelude::v1::*;

use std::cell::{UnsafeCell, Cell};
use std::marker;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

scoped_thread_local!(static CURRENT_TASK: Task);
scoped_thread_local!(static CURRENT_UNPARK: Arc<Unpark>);

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
/// # Panics
///
/// This function will panic if a future is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
pub fn park() -> Arc<Unpark> {
    CURRENT_UNPARK.with(|handle| handle.clone())
}

/// Marks the current task as ready to be polled again immediately, even if the
/// current `poll` returns `NotReady`.
///
/// Useful for cooperative scheduling: you can give the executor a chance to
/// make progress on other tasks, then poll this task again later.
///
/// # Panics
///
/// This function will panic if a future is not currently being executed. That
/// is, this method can be dangerous to call outside of an implementation of
/// `poll`.
pub fn yield_now() {
    CURRENT_TASK.with(|task| task.should_repoll.set(true))
}

/// A structure representing one "task", or lightweight thread of execution
/// throughout the lifetime of a set of futures.
///
/// It's intended that futures are composed together to form a large "task" of
/// futures which is driven as a whole throughout its lifetime. This task is
/// persistent for the entire lifetime of the future until its completion. It
/// specifies *how* and *where* the task is executed, e.g. on a thread pool, an
/// event loop or wherever it happens to be woken (via `Inline`).
///
/// Tasks also provide a place to store arbitrary data, through `TaskRc`,
/// which can be shared freely amongst the futures making up the task.
pub struct Task {
    id: usize,
    should_repoll: Cell<bool>,

    // A `Task` is not `Sync`, see the TaskRc docs below above.
    _marker: marker::PhantomData<Cell<()>>,
}

static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;

impl Task {
    /// Creates a new task.
    pub fn new() -> Task {
        Task {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            should_repoll: Cell::new(false),
            _marker: marker::PhantomData,
        }
    }

    /// Did the underlying future request to yield?
    pub fn should_repoll(&self) -> bool {
        self.should_repoll.get()
    }

    fn inner_usize(&self) -> usize {
        self.id
    }

    /// Sets the global running task and unpark handle for the duration of
    /// the provided closure.
    ///
    /// This function will configure the current task to be this task itself and
    /// then call the closure provided. For the duration of the closure the
    /// "current task" will be set to this task, and then after the closure
    /// returns the current task will be reset to what it was before.
    pub fn enter<F, R>(&mut self, handle: &Arc<Unpark>, f: F) -> R
        where F: FnOnce() -> R,
    {
        CURRENT_TASK.set(self, || {
            CURRENT_UNPARK.set(&handle, f)
        })
    }
}

/// A trait for notifying a task that it should "wake up" and poll its future.
///
/// This is a trait because the mechanism for wakeup will vary by task executor.
pub trait Unpark: Send + Sync {
    /// Notify the associated task that a future is ready to get polled.
    ///
    /// Futures should use this method to ensure that when a future can make
    /// progress its `Task` is notified that it should continue to `poll` the
    /// future at a later date.
    ///
    /// It must be guaranteed that if `unpark` is called that `poll` will be
    /// called at some later point by the task (unless the task's future is
    /// complete). If the task is currently polling its future, it must poll the
    /// future *again*, ensuring that all relevant events are eventually
    /// observed by the future.
    fn unpark(&self);
}

// One critical piece of this module's contents are the `TaskRc<A>` handles.
// The purpose of this is to conceptually be able to store data in a task,
// allowing it to be accessed within multiple futures at once. For example if
// you have some concurrent futures working, they may all want mutable access to
// some data. We already know that when the futures are being poll'ed that we're
// entirely synchronized (aka `&mut Task`), so you shouldn't require an
// `Arc<Mutex<T>>` to share as the synchronization isn't necessary!
//
// So the idea here is that you insert data into a task via `Task::insert`, and
// a handle to that data is then returned to you. That handle can later get
// presented to the task itself to actually retrieve the underlying data. The
// invariant is that the data can only ever be accessed with the task present,
// and the lifetime of the actual data returned is connected to the lifetime of
// the task itself.
//
// Conceptually I at least like to think of this as "dynamically adding more
// struct fields to a `Task`". Each call to insert creates a new "name" for the
// struct field, a `TaskRc<A>`, and then you can access the fields of a struct
// with the struct itself (`Task`) as well as the name of the field
// (`TaskRc<A>`). If that analogy doesn't make sense then oh well, it at least
// helped me!
//
// So anyway, we do some interesting trickery here to actually get it to work.
// Each `TaskRc<A>` handle stores `Arc<UnsafeCell<A>>`. So it turns out, we're
// not even adding data to the `Task`! Each `TaskRc<A>` contains a reference
// to this `Arc`, and `TaskRc` handles can be cloned which just bumps the
// reference count on the `Arc` itself.
//
// As before, though, you can present the `Arc` to a `Task` and if they
// originated from the same place you're allowed safe access to the internals.
// We allow but shared and mutable access without the `Sync` bound on the data,
// crucially noting that a `Task` itself is not `Sync`.
//
// So hopefully I've convinced you of this point that the `get` and `get_mut`
// methods below are indeed safe. The data is always valid as it's stored in an
// `Arc`, and access is only allowed with the proof of the associated `Task`.
// One thing you might be asking yourself though is what exactly is this "proof
// of a task"? Right now it's a `usize` corresponding to the `Task`'s
// `TaskHandle` arc allocation.
//
// Wait a minute, isn't that the ABA problem! That is, we create a task A, add
// some data to it, destroy task A, do some work, create a task B, and then ask
// to get the data from task B. In this case though the point of the
// `task_inner` "proof" field is simply that there's some non-`Sync` token
// proving that you can get access to the data. So while weird, this case should
// still be safe, as the data's not stored in the task itself.

/// A reference to a piece of data that's accessible only within a specific
/// `Task`.
///
/// This data is `Send` even when `A` is not `Sync`, because the data stored
/// within is accessed in a single-threaded way. The thread accessing it may
/// change over time, if the task migrates, so `A` must be `Send`.
pub struct TaskRc<A> {
    task_inner: usize,
    ptr: Arc<UnsafeCell<A>>,
}

// for safety here, see docs at the top of this module
unsafe impl<A: Send> Send for TaskRc<A> {}
unsafe impl<A: Sync> Sync for TaskRc<A> {}

impl<A> TaskRc<A> {
    /// Inserts a new piece of task-local data into this task, returning a
    /// reference to it.
    ///
    /// Ownership of the data will be transferred to the task, and the data will
    /// be destroyed when the task itself is destroyed. The returned value can
    /// be passed to the `with` method to get a reference back to the original
    /// data.
    ///
    /// Note that the returned handle is cloneable and copyable and can be sent
    /// to other futures which will be associated with the same task. All
    /// futures will then have access to this data when passed the reference
    /// back.
    ///
    /// # Panics
    ///
    /// This function will panic if a task is not currently running.
    pub fn new(a: A) -> TaskRc<A> {
        CURRENT_TASK.with(|task| {
            TaskRc {
                task_inner: task.inner_usize(),
                ptr: Arc::new(UnsafeCell::new(a)),
            }
        })
    }

    /// Operate with a reference to the underlying data.
    ///
    /// This method should be passed a handle previously returned by
    /// `Task::insert`. That handle, when passed back into this method, will
    /// retrieve a reference to the original data.
    ///
    /// # Panics
    ///
    /// This method will panic if a task is not currently running or if `self`
    /// does not belong to the task that is currently running. That is, if
    /// another task generated the `data` handle passed in, this method will
    /// panic.
    pub fn with<F, R>(&self, f: F) -> R
        where F: FnOnce(&A) -> R
    {
        // for safety here, see docs at the top of this module
        CURRENT_TASK.with(|task| {
            assert_eq!(self.task_inner, task.inner_usize());
            f(unsafe { &*self.ptr.get() })
        })
    }
}

impl<A> Clone for TaskRc<A> {
    fn clone(&self) -> TaskRc<A> {
        TaskRc {
            task_inner: self.task_inner,
            ptr: self.ptr.clone(),
        }
    }
}
