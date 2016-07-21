#![allow(missing_docs)]

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

pub struct Task {
    id: usize,
    ready: usize,
    list: Box<Any + Send>,
    handle: TaskHandle,
    tokens: Tokens,
}

pub struct ScopedTask<'a> {
    task: &'a mut Task,
    reset: bool,
}

#[derive(Clone)]
pub struct TaskHandle {
    inner: Arc<Inner>,
}

struct Inner {
    slot: Slot<(Task, Box<Future<Item=(), Error=()>>)>,
    tokens: Tokens,
    registered: AtomicBool,
}

pub struct TaskData<A: Send + 'static> {
    id: usize,
    ptr: *mut A,
}

impl Task {
    pub fn new() -> Task {
        static NEXT: AtomicUsize = ATOMIC_USIZE_INIT;

        Task {
            // TODO: what to do if this overflows?
            id: NEXT.fetch_add(1, Ordering::SeqCst),
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

    pub fn may_contain(&self, token: usize) -> bool {
        self.ready > 0 || self.tokens.may_contain(token)
    }

    pub fn insert<A>(&mut self, a: A) -> TaskData<A>
        where A: Any + Send + 'static,
    {
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

    pub fn get<A>(&self, data: &TaskData<A>) -> &A
        where A: Send + 'static,
    {
        assert_eq!(data.id, self.id);
        unsafe { &*data.ptr }
    }

    pub fn get_mut<A>(&mut self, data: &TaskData<A>) -> &mut A
        where A: Send + 'static,
    {
        assert_eq!(data.id, self.id);
        unsafe { &mut *data.ptr }
    }

    pub fn handle(&self) -> &TaskHandle {
        &self.handle
    }

    // TODO: terrible name
    pub fn scoped(&mut self) -> ScopedTask {
        ScopedTask { task: self, reset: false }
    }

    pub fn run(self, mut future: Box<Future<Item=(), Error=()>>) {
        let mut me = self;
        loop {
            // Note that we need to poll at least once as the wake callback may
            // have received an empty set of tokens, but that's still a valid
            // reason to poll a future.
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

        // TODO: don't clone if we've been scheduled immediately
        let inner = me.handle.inner.clone();
        inner.slot.try_produce((me, future)).ok().unwrap();
    }

    pub fn notify(&mut self) {
        // TODO: optimize this
        self.handle().notify()
    }
}

fn catch_unwind<F, U>(f: F) -> thread::Result<U>
    where F: FnOnce() -> U + Send + 'static,
{
    panic::catch_unwind(panic::AssertUnwindSafe(f))
}

impl TaskHandle {
    pub fn token_ready(&self, token: usize) {
        self.inner.tokens.insert(token);
    }

    pub fn equivalent(&self, other: &TaskHandle) -> bool {
        &*self.inner as *const _ == &*other.inner as *const _
    }

    pub fn notify(&self) {
        // Next, see if we can actually register an `on_full` callback. The
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
            task.run(future)
        });
    }
}

impl<'a> ScopedTask<'a> {
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

impl<A: Send + 'static> Clone for TaskData<A> {
    fn clone(&self) -> TaskData<A> {
        TaskData {
            id: self.id,
            ptr: self.ptr,
        }
    }
}

impl<A: Send + 'static> Copy for TaskData<A> {}
