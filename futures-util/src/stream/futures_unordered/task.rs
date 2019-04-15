use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicPtr, AtomicBool};
use core::sync::atomic::Ordering::SeqCst;
use alloc::sync::{Arc, Weak};

use crate::task::{ArcWake, WakerRef, waker_ref};
use super::ReadyToRunQueue;
use super::abort::abort;

pub(super) struct Task<Fut> {
    // The future
    pub(super) future: UnsafeCell<Option<Fut>>,

    // Next pointer for linked list tracking all active tasks
    pub(super) next_all: UnsafeCell<*const Task<Fut>>,

    // Previous task in linked list tracking all active tasks
    pub(super) prev_all: UnsafeCell<*const Task<Fut>>,

    // Next pointer in ready to run queue
    pub(super) next_ready_to_run: AtomicPtr<Task<Fut>>,

    // Queue that we'll be enqueued to when woken
    pub(super) ready_to_run_queue: Weak<ReadyToRunQueue<Fut>>,

    // Whether or not this task is currently in the ready to run queue
    pub(super) queued: AtomicBool,
}

// `Task` can be sent across threads safely because it ensures that
// the underlying `Fut` type isn't touched from any of its methods.
//
// The parent (`super`) module is trusted not to access `future`
// across different threads.
unsafe impl<Fut> Send for Task<Fut> {}
unsafe impl<Fut> Sync for Task<Fut> {}

impl<Fut> ArcWake for Task<Fut> {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let inner = match arc_self.ready_to_run_queue.upgrade() {
            Some(inner) => inner,
            None => return,
        };

        // It's our job to enqueue this task it into the ready to run queue. To
        // do this we set the `queued` flag, and if successful we then do the
        // actual queueing operation, ensuring that we're only queued once.
        //
        // Once the task is inserted call `wake` to notify the parent task,
        // as it'll want to come along and run our task later.
        //
        // Note that we don't change the reference count of the task here,
        // we merely enqueue the raw pointer. The `FuturesUnordered`
        // implementation guarantees that if we set the `queued` flag that
        // there's a reference count held by the main `FuturesUnordered` queue
        // still.
        let prev = arc_self.queued.swap(true, SeqCst);
        if !prev {
            inner.enqueue(&**arc_self);
            inner.waker.wake();
        }
    }
}

impl<Fut> Task<Fut> {
    /// Returns a waker reference for this task without cloning the Arc.
    pub(super) fn waker_ref<'a>(this: &'a Arc<Task<Fut>>) -> WakerRef<'a> {
        waker_ref(this)
    }
}

impl<Fut> Drop for Task<Fut> {
    fn drop(&mut self) {
        // Since `Task<Fut>` is sent across all threads for any lifetime,
        // regardless of `Fut`, we, to guarantee memory safety, can't actually
        // touch `Fut` at any time except when we have a reference to the
        // `FuturesUnordered` itself .
        //
        // Consequently it *should* be the case that we always drop futures from
        // the `FuturesUnordered` instance. This is a bomb, just in case there's
        // a bug in that logic.
        unsafe {
            if (*self.future.get()).is_some() {
                abort("future still here when dropping");
            }
        }
    }
}
