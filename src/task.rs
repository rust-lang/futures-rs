
// One critical piece of this module's contents are the `TaskData<A>` handles.
// The purpose of this is to conceptually be able to store data in a task,
// allowing it to be accessed within multiple futures at once. For example if
// you have some concurrent futures working, they may all want mutable access to
// some data. We already know that when the futures are being poll'd that we're
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
// struct field, a `TaskData<A>`, and then you can access the fields of a struct
// with the struct itself (`Task`) as well as the name of the field
// (`TaskData<A>`). If that analogy doesn't make sense then oh well, it at least
// helped me!
//
// So anyway, we do some interesting trickery here to actually get it to work.
// Each `TaskData<A>` handle stores `Arc<UnsafeCell<A>>`. So it turns out, we're
// not even adding data to the `Task`! Each `TaskData<A>` contains a reference
// to this `Arc`, and `TaskData` handles can be cloned which just bumps the
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

use std::any::Any;
use std::cell::{UnsafeCell, Cell};
use std::marker;
use std::panic;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, ATOMIC_USIZE_INIT, Ordering};
use std::thread;

use Future;
use executor::{DEFAULT, Executor};
use slot::Slot;

/// A structure representing one "task", or thread of execution throughout the
/// lifetime of a set of futures.
///
/// It's intended that futures are composed together to form a large "task" of
/// futures which is driven as a whole throughout its lifetime. This task is
/// persistent for the entire lifetime of the future until its completion,
/// carrying any local data and such.
///
/// Currently tasks serve two primary purposes:
///
/// * They're used to drive futures to completion, e.g. executors (more to be
///   changed here soon).
/// * They store task local data. That is, any task can contain any number of
///   pieces of arbitrary data which can be accessed at a later date. The data
///   is owned and carried in the task itself, and `TaskData` handles are used
///   to access the internals.
///
/// This structure is likely to expand more customizable functionality over
/// time! That is, it's not quite done yet...
pub struct Task {
    handle: TaskHandle,

    // A `Task` is not `Sync`, see the docs above.
    _marker: marker::PhantomData<Cell<()>>,
}

/// A handle to a task that can be sent to other threads.
///
/// Created by the `Task::handle` method.
#[derive(Clone)]
pub struct TaskHandle {
    inner: Arc<Inner>,
}

struct Inner {
    slot: Slot<(Task, Box<Future<Item=(), Error=()>>)>,
    registered: AtomicBool,
}

/// A reference to a piece of data that's stored inside of a `Task`.
///
/// This can be used with the `Task::get` and `Task::get_mut` methods to access
/// data inside of tasks.
pub struct TaskData<A> {
    task_inner: usize,
    ptr: Arc<UnsafeCell<A>>,
}

// for safety here, see docs at the top of this module
unsafe impl<A: Send> Send for TaskData<A> {}
unsafe impl<A: Sync> Sync for TaskData<A> {}

impl Task {
    /// Creates a new task ready to drive a future.
    pub fn new() -> Task {
        Task {
            handle: TaskHandle {
                inner: Arc::new(Inner {
                    slot: Slot::new(None),
                    registered: AtomicBool::new(false),
                }),
            },
            _marker: marker::PhantomData,
        }
    }

    /// Inserts a new piece of task-local data into this task, returning a
    /// reference to it.
    ///
    /// Ownership of the data will be transferred to the task, and the data will
    /// be destroyed when the task itself is destroyed. The returned value can
    /// be passed to the `Task::{get, get_mut}` methods to get a reference back
    /// to the original data.
    ///
    /// Note that the returned handle is cloneable and copyable and can be sent
    /// to other futures which will be associated with the same task. All
    /// futures will then have access to this data when passed the reference
    /// back.
    pub fn insert<A>(&mut self, a: A) -> TaskData<A>
        where A: Any + Send + 'static,
    {
        TaskData {
            task_inner: self.inner_usize(),
            ptr: Arc::new(UnsafeCell::new(a)),
        }
    }

    fn inner_usize(&self) -> usize {
        &*self.handle.inner as *const Inner as usize
    }

    /// Get a reference to the task-local data inside this task.
    ///
    /// This method should be passed a handle previously returned by
    /// `Task::insert`. That handle, when passed back into this method, will
    /// retrieve a reference to the original data.
    ///
    /// # Panics
    ///
    /// This method will panic if `data` does not belong to this task. That is,
    /// if another task generated the `data` handle passed in, this method will
    /// panic.
    pub fn get<A>(&self, data: &TaskData<A>) -> &A {
        // for safety here, see docs at the top of this module
        assert_eq!(data.task_inner, self.inner_usize());
        unsafe { &*data.ptr.get() }
    }

    /// Get a mutable reference to the task-local data inside this task.
    ///
    /// This method should be passed a handle previously returned by
    /// `Task::insert`. That handle, when passed back into this method, will
    /// retrieve a reference to the original data.
    ///
    /// # Panics
    ///
    /// This method will panic if `data` does not belong to this task. That is,
    /// if another task generated the `data` handle passed in, this method will
    /// panic.
    pub fn get_mut<A>(&mut self, data: &TaskData<A>) -> &mut A {
        // for safety here, see docs at the top of this module
        assert_eq!(data.task_inner, self.inner_usize());
        unsafe { &mut *data.ptr.get() }
    }

    /// During the `Future::schedule` method, notify to the task that a value is
    /// immediately ready.
    ///
    /// This method, more optimized than `TaskHandle::notify`, will inform the
    /// task that the future which is being scheduled is immediately ready to be
    /// `poll`ed again.
    pub fn notify(&mut self) {
        // TODO: optimize this, we've got mutable access so no need for atomics
        self.handle().notify()
    }

    /// Gets a handle to this task which can be cloned to a piece of
    /// `Send+'static` data.
    ///
    /// This handle returned can be used to notify the task that a future is
    /// ready to get polled again. The returned handle implements the `Clone`
    /// trait and all clones will refer to this same task.
    ///
    /// Note that if data is immediately ready then the `Task::notify` method
    /// should be preferred.
    pub fn handle(&self) -> &TaskHandle {
        &self.handle
    }

    /// Consumes this task to run a future to completion.
    ///
    /// This function will consume the task provided and the task will be used
    /// to execute the `future` provided until it has been completed. The future
    /// wil be `poll`'ed until it is resolved, at which point the `Result<(),
    /// ()>` will be discarded.
    ///
    /// The future will be `poll`ed on the threads that events arrive on. That
    /// is, this method does not attempt to control which thread a future is
    /// polled on.
    ///
    /// # Panics
    ///
    /// Currently, if `poll` panics, then this method will propagate the panic
    /// to the thread that `poll` was called on. This is bad and it will change.
    pub fn run(self, mut future: Box<Future<Item=(), Error=()>>) {
        let mut me = self;
        loop {
            // Note that we need to poll at least once as the wake callback may
            // have received an empty set of tokens, but that's still a valid
            // reason to poll a future.
            let result = catch_unwind(move || {
                (future.poll(&mut me), future, me)
            });
            match result {
                Ok((ref r, _, _)) if r.is_ready() => return,
                Ok((_, f, t)) => {
                    future = f;
                    me = t;
                }
                // TODO: this error probably wants to get communicated to
                //       another closure in one way or another, or perhaps if
                //       nothing is registered the panic propagates.
                Err(e) => panic::resume_unwind(e),
            }
            future = match future.tailcall() {
                Some(f) => f,
                None => future,
            };
            break
        }

        // Ok, we've seen that there are no tokens which show interest in the
        // future. Schedule interest on the future for when something is ready
        // and then relinquish the future and the forget back to the slot, which
        // will then pick it up once a wake callback has fired.
        future.schedule(&mut me);

        let inner = me.handle.inner.clone();
        inner.slot.try_produce((me, future)).ok().unwrap();
    }
}

fn catch_unwind<F, U>(f: F) -> thread::Result<U>
    where F: FnOnce() -> U + Send + 'static,
{
    panic::catch_unwind(panic::AssertUnwindSafe(f))
}

impl TaskHandle {
    /// Returns whether this task handle and another point to the same task.
    ///
    /// In other words, this method returns whether `notify` would end up
    /// notifying the same task. If two task handles need to be notified but
    /// they are equivalent, then only one needs to be actually notified.
    pub fn equivalent(&self, other: &TaskHandle) -> bool {
        &*self.inner as *const _ == &*other.inner as *const _
    }

    /// Notify the associated task that a future is ready to get polled.
    ///
    /// Futures should use this method to ensure that when a future can make
    /// progress as `Task` is notified that it should continue to `poll` the
    /// future at a later date.
    ///
    /// Currently it's guaranteed that if `notify` is called that `poll` will be
    /// scheduled to get called at some point in the future. A `poll` may
    /// already be running on another thread, but this will ensure that a poll
    /// happens again to receive this notification.
    pub fn notify(&self) {
        // First, see if we can actually register an `on_full` callback. The
        // `Slot` requires that only one registration happens, and this flag
        // guards that.
        if self.inner.registered.swap(true, Ordering::SeqCst) {
            return
        }

        // If we won the race to register a callback, do so now. Once the slot
        // is resolve we allow another registration **before we poll again**.
        // This allows any future which may be somewhat badly behaved to be
        // compatible with this.
        self.inner.slot.on_full(|slot| {
            let (task, future) = slot.try_consume().ok().unwrap();
            task.handle.inner.registered.store(false, Ordering::SeqCst);
            DEFAULT.execute(|| task.run(future))
        });
    }
}

impl<A> Clone for TaskData<A> {
    fn clone(&self) -> TaskData<A> {
        TaskData {
            task_inner: self.task_inner,
            ptr: self.ptr.clone(),
        }
    }
}
