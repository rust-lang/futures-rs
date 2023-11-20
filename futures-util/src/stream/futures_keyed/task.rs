use alloc::sync::{Arc, Weak};
use core::borrow::Borrow;
use core::cell::UnsafeCell;
use core::ops::Deref;
use core::sync::atomic::Ordering::{self, Relaxed, SeqCst};
use core::sync::atomic::{AtomicBool, AtomicPtr};

use super::abort::abort;
use super::ReadyToRunQueue;
use crate::task::{waker_ref, ArcWake, WakerRef};
use core::hash::Hash;

pub(crate) struct Task<K, Fut> {
    // The future
    pub(crate) future: UnsafeCell<Option<Fut>>,

    // The key
    pub(crate) key: UnsafeCell<Option<K>>,

    // Next pointer for linked list tracking all active tasks (use
    // `spin_next_all` to read when access is shared across threads)
    pub(super) next_all: AtomicPtr<Task<K, Fut>>,

    // Previous task in linked list tracking all active tasks
    pub(super) prev_all: UnsafeCell<*const Task<K, Fut>>,

    // Length of the linked list tracking all active tasks when this node was
    // inserted (use `spin_next_all` to synchronize before reading when access
    // is shared across threads)
    pub(super) len_all: UnsafeCell<usize>,

    // Next pointer in ready to run queue
    pub(super) next_ready_to_run: AtomicPtr<Task<K, Fut>>,

    // Queue that we'll be enqueued to when woken
    pub(super) ready_to_run_queue: Weak<ReadyToRunQueue<K, Fut>>,

    // Whether or not this task is currently in the ready to run queue
    pub(super) queued: AtomicBool,

    // Whether the future was awoken during polling
    // It is possible for this flag to be set to true after the polling,
    // but it will be ignored.
    pub(super) woken: AtomicBool,
}

// `Task` can be sent across threads safely because it ensures that
// the underlying `Fut` type isn't touched from any of its methods.
//
// The parent (`super`) module is trusted not to access `future`
// across different threads.
unsafe impl<K, Fut> Send for Task<K, Fut> {}
unsafe impl<K, Fut> Sync for Task<K, Fut> {}

impl<K, Fut> ArcWake for Task<K, Fut> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let inner = match arc_self.ready_to_run_queue.upgrade() {
            Some(inner) => inner,
            None => return,
        };

        arc_self.woken.store(true, Relaxed);

        // It's our job to enqueue this task it into the ready to run queue. To
        // do this we set the `queued` flag, and if successful we then do the
        // actual queueing operation, ensuring that we're only queued once.
        //
        // Once the task is inserted call `wake` to notify the parent task,
        // as it'll want to come along and run our task later.
        //
        // Note that we don't change the reference count of the task here,
        // we merely enqueue the raw pointer. The `FuturesKeyed`
        // implementation guarantees that if we set the `queued` flag that
        // there's a reference count held by the main `FuturesKeyed` queue
        // still.
        let prev = arc_self.queued.swap(true, SeqCst);
        if !prev {
            inner.enqueue(Arc::as_ptr(arc_self));
            inner.waker.wake();
        }
    }
}

impl<K, Fut> Task<K, Fut> {
    /// Returns a waker reference for this task without cloning the Arc.
    pub(super) fn waker_ref(this: &Arc<Self>) -> WakerRef<'_> {
        waker_ref(this)
    }

    /// Spins until `next_all` is no longer set to `pending_next_all`.
    ///
    /// The temporary `pending_next_all` value is typically overwritten fairly
    /// quickly after a node is inserted into the list of all futures, so this
    /// should rarely spin much.
    ///
    /// When it returns, the correct `next_all` value is returned.
    ///
    /// `Relaxed` or `Acquire` ordering can be used. `Acquire` ordering must be
    /// used before `len_all` can be safely read.
    #[inline]
    pub(super) fn spin_next_all(
        &self,
        pending_next_all: *mut Self,
        ordering: Ordering,
    ) -> *const Self {
        loop {
            let next = self.next_all.load(ordering);
            if next != pending_next_all {
                return next;
            }
        }
    }
    pub(crate) fn key(&self) -> Option<&K> {
        unsafe { (&*self.key.get()).as_ref() }
    }
    pub(crate) fn take_key(&self) -> K {
        unsafe { (*self.key.get()).take().unwrap() }
    }
}

impl<K, Fut> Drop for Task<K, Fut> {
    fn drop(&mut self) {
        // Since `Task<K,Fut>` is sent across all threads for any lifetime,
        // regardless of `Fut`, we, to guarantee memory safety, can't actually
        // touch `Fut` at any time except when we have a reference to the
        // `FuturesKeyed` itself .
        //
        // Consequently it *should* be the case that we always drop futures from
        // the `FuturesKeyed` instance. This is a bomb, just in case there's
        // a bug in that logic.
        unsafe {
            if (*self.future.get()).is_some() {
                abort("future still here when dropping");
            }
        }
    }
}

// Wrapper struct; exists effectively to implement hash on the type Arc<Task>
pub(crate) struct HashTask<K: Hash + Eq, Fut> {
    pub(crate) inner: Arc<Task<K, Fut>>,
}

impl<K: Hash + Eq, F> From<Arc<Task<K, F>>> for HashTask<K, F> {
    fn from(inner: Arc<Task<K, F>>) -> Self {
        HashTask { inner }
    }
}

impl<K: Hash + Eq, Fut> HashTask<K, Fut> {
    fn key(&self) -> Option<&K> {
        Task::key(&*self)
    }
    // pub(crate) fn key_unwrap(&self) -> &K {
    //     unsafe { (&*self.key.get()).as_ref().unwrap() }
    // }
}

impl<K: Hash + Eq, Fut> Deref for HashTask<K, Fut> {
    type Target = Task<K, Fut>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<K: Hash + Eq, Fut> Borrow<K> for HashTask<K, Fut> {
    fn borrow(&self) -> &K {
        // Never use the borrowed form after the key has been removed from the task
        // IE. The Stub task never goes into the HashSet
        // Or removing Task from HashSet using key, after key removed from task
        unsafe { (*self.key.get()).as_ref().unwrap() }
    }
}

impl<K: Hash + Eq, Fut> Hash for HashTask<K, Fut> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        unsafe { (*self.key.get()).as_ref() }.unwrap().hash(state)
    }
}

impl<K: Hash + Eq, Fut> PartialEq for HashTask<K, Fut> {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl<K: Hash + Eq, Fut> Eq for HashTask<K, Fut> {}
unsafe impl<K: Hash + Eq, Fut> Send for HashTask<K, Fut> {}
unsafe impl<K: Hash + Eq, Fut> Sync for HashTask<K, Fut> {}
