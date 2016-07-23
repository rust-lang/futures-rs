use std::any::Any;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::panic;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, ATOMIC_USIZE_INIT, Ordering};
use std::thread;

use Future;
use token::Tokens;
use slot::Slot;
use executor::{DEFAULT, Executor};

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
    id: usize,
    ready: usize,
    list: Box<Any + Send>,
    handle: TaskHandle,
    tokens: Tokens,
}

/// A scoped version of a task, returned by the `Task::scoped` method.
pub struct ScopedTask<'a> {
    task: &'a mut Task,
    reset: bool,
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
    tokens: Tokens,
    registered: AtomicBool,
}

/// A reference to a piece of data that's stored inside of a `Task`.
///
/// This can be used with the `Task::get` and `Task::get_mut` methods to access
/// data inside of tasks.
pub struct TaskData<A> {
    id: usize,
    ptr: *mut A,
}

unsafe impl<A: Send> Send for TaskData<A> {}
unsafe impl<A: Sync> Sync for TaskData<A> {}

impl Task {
    /// Creates a new task ready to drive a future.
    ///
    /// The returned task has no internal data stored in it and considers all
    /// tokens "ready for polling" until informed otherwise.
    pub fn new() -> Task {
        static NEXT: AtomicUsize = ATOMIC_USIZE_INIT;

        // TODO: what to do if this overflows?
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        assert!(id != usize::max_value());

        Task {
            id: id,
            list: Box::new(()),
            tokens: Tokens::all(),
            ready: 0,
            handle: TaskHandle {
                inner: Arc::new(Inner {
                    slot: Slot::new(None),
                    registered: AtomicBool::new(false),
                    tokens: Tokens::all(),
                }),
            },
        }
    }

    /// Tests whether a token may be ready for polling.
    ///
    /// As events come in interest in particular tokens can be signaled through
    /// the `TaskHandle::token_ready` method. This method can then be used to
    /// see whether a token has arrive yet or not.
    ///
    /// Note that this will never return a false negative, but it can return
    /// false positives. That is, if a token has been passed to `token_ready`,
    /// that exact token will always return `true`. If a token has not been
    /// passed to `token_ready`, though, it may still return `true`. (e.g. this
    /// is a bloom filter).
    // TODO: probably want an opaque type around usize
    // TODO: needs a better name
    pub fn may_contain(&self, token: usize) -> bool {
        self.ready > 0 || self.tokens.may_contain(token)
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
        // Right now our list of task-local data is just stored as a linked
        // list, so allocate a new node and insert it into the list.
        struct Node<T: ?Sized> {
            _next: Box<Any + Send>,
            data: T,
        }
        let prev = mem::replace(&mut self.list, Box::new(()));
        let mut next = Box::new(Node { _next: prev, data: a });
        let ret = TaskData { id: self.id, ptr: &mut next.data };
        self.list = next;
        return ret
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
        assert_eq!(data.id, self.id);
        unsafe { &*data.ptr }
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
        assert_eq!(data.id, self.id);
        unsafe { &mut *data.ptr }
    }

    /// During the `Future::schedule` method, notify to the task that a value is
    /// immediately ready.
    ///
    /// This method, more optimized than `TaskHandle::notify`, will inform the
    /// task that the future which is being scheduled is immediately ready to be
    /// `poll`ed again.
    pub fn notify(&mut self) {
        // TODO: optimize this
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

    /// Creates a new temporary "scoped task" for setting the ready tokens.
    ///
    /// A scoped task is simply a borrowed version of this task with an
    /// associated destructor that restores the ready set when it goes out of
    /// scope. This returned object can use the `ScopedTask::ready` method to
    /// indicate that all further calls to `Task::may_contain` will return true.
    ///
    /// This can be useful when one call to `poll` may poll multiple futures or
    /// streams, but after one has been resolved the others may all immediately
    /// be ready so they shouldn't use the same readiness token sets as the
    /// previous one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use futures::Task;
    ///
    /// fn poll(task: &mut Task) {
    ///     task.may_contain(1); // may return true or false
    ///
    ///     {
    ///         let mut task = task.scoped();
    ///
    ///         task.may_contain(1); // returns the same as before
    ///
    ///         task.ready();
    ///         assert!(task.may_contain(1)); // always returns true
    ///     }
    ///
    ///     task.may_contain(1); // returns the same as the first call
    /// }
    /// ```
    // TODO: terrible name
    pub fn scoped(&mut self) -> ScopedTask {
        ScopedTask { task: self, reset: false }
    }

    /// Consumes this task to run a future to completion.
    ///
    /// This function will consume the task provided and the task will be used
    /// to execute the `future` provided until it has been completed. The future
    /// wil be `poll`'d until it is resolved, at which point the `Result<(),
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
            assert_eq!(me.ready, 0);
            me.tokens = me.handle.inner.tokens.take();
            let result = catch_unwind(move || {
                (future.poll(&mut me), future, me)
            });
            match result {
                Ok((ref r, _, _)) if r.is_ready() => return,
                Ok((_, f, t)) => {
                    future = f;
                    me = t;
                }
                // TODO: do something smarter
                Err(e) => panic::resume_unwind(e),
            }
            future = match future.tailcall() {
                Some(f) => f,
                None => future,
            };
            if !me.handle.inner.tokens.any() {
                break
            }
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

    /// Inform this handle's associated task that a particular token is ready.
    ///
    /// This information will be communicated to the task in question and
    /// affects the return value of future calls to `Task::may_contain`. This
    /// can be used to optimize calls to `poll` by avoiding a polling operation
    /// if `may_contain` returns false.
    pub fn token_ready(&self, token: usize) {
        self.inner.tokens.insert(token);
    }

    /// Notify the associated task that a future is ready to get polled.
    ///
    /// Futures should use this method to ensure that when a future can make
    /// progress as `Task` is notified that it should continue to `poll` the
    /// future at a later date.
    // TODO: more here
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
        //
        // TODO: this store of `false` should *probably* be before the
        //       `schedule` call in forget above, need to think it through.
        self.inner.slot.on_full(|slot| {
            let (task, future) = slot.try_consume().ok().unwrap();
            task.handle.inner.registered.store(false, Ordering::SeqCst);
            DEFAULT.execute(|| task.run(future))
        });
    }
}

impl<'a> ScopedTask<'a> {
    /// Flag this scoped task as "all events are ready".
    ///
    /// In other words, once this method is called, all future calls to
    /// `may_contain` on the associated `Task` will return `true`.
    pub fn ready(&mut self) -> &mut ScopedTask<'a>{
        if !self.reset {
            self.reset = true;
            self.task.ready += 1;
        }
        self
    }
}

impl<'a> Deref for ScopedTask<'a> {
    type Target = Task;
    fn deref(&self) -> &Task {
        &*self.task
    }
}

impl<'a> DerefMut for ScopedTask<'a> {
    fn deref_mut(&mut self) -> &mut Task {
        &mut *self.task
    }
}

impl<'a> Drop for ScopedTask<'a> {
    fn drop(&mut self) {
        if self.reset {
            self.task.ready -= 1;
        }
    }
}

impl<A> Clone for TaskData<A> {
    fn clone(&self) -> TaskData<A> {
        TaskData {
            id: self.id,
            ptr: self.ptr,
        }
    }
}

impl<A> Copy for TaskData<A> {}
