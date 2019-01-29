use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem;
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicPtr, AtomicBool};
use core::sync::atomic::Ordering::SeqCst;
use alloc::sync::{Arc, Weak};

use futures_core::task::{UnsafeWake, Waker, LocalWaker};

use crate::task::LocalWakerRef;
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

impl<Fut> Task<Fut> {
    pub(super) fn wake(this: &Arc<Task<Fut>>) {
        let inner = match this.ready_to_run_queue.upgrade() {
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
        let prev = this.queued.swap(true, SeqCst);
        if !prev {
            inner.enqueue(&**this);
            inner.waker.wake();
        }
    }

    /// Returns a waker.
    pub(super) fn waker(this: &Arc<Task<Fut>>) -> Waker {
        let clone = this.clone();

        // Safety: This is save because an `Arc` is a struct which contains
        // a single field that is a pointer.
        let ptr = unsafe {
            mem::transmute::<Arc<Task<Fut>>,
                             NonNull<ArcTask<Fut>>>(clone)
        };

        let ptr = ptr as NonNull<dyn UnsafeWake>;

        // Hide lifetime of `Fut`
        // Safety: The waker can safely outlive the future because the
        // `UnsafeWake` impl is guaranteed to not touch `Fut`.
        let ptr = unsafe {
            mem::transmute::<NonNull<dyn UnsafeWake>,
                             NonNull<dyn UnsafeWake>>(ptr)
        };

        unsafe { Waker::new(ptr) }
    }

    /// Returns a local waker for this task without cloning the Arc.
    pub(super) fn local_waker<'a>(this: &'a Arc<Task<Fut>>) -> LocalWakerRef<'a> {
        // Safety: This is safe because an `Arc` is a struct which contains
        // a single field that is a pointer.
        let ptr = unsafe {
            *(this as *const _ as *const NonNull<ArcTaskUnowned<Fut>>)
        };

        let ptr = ptr as NonNull<dyn UnsafeWake>;

        // Hide lifetime of `self`
        // Safety:
        // - Since the `Arc` has not been cloned, the local waker must
        //   not outlive it. This is ensured by the lifetime of `LocalWakerRef`.
        // - The local waker can safely outlive the future because the
        //   `UnsafeWake` impl is guaranteed to not touch `Fut`.
        unsafe {
            let ptr = mem::transmute::<NonNull<dyn UnsafeWake>,
                                       NonNull<dyn UnsafeWake>>(ptr);
            LocalWakerRef::new(LocalWaker::new(ptr))
        }
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

// `ArcTask<Fut>` represents conceptually the struct an `Arc<Task<Fut>>` points
// to. `*const ArcTask<Fut>` is equal to `Arc<Task<Fut>>`
// It may only be used through references because its layout obviously doesn't
// match the real inner struct of an `Arc` which (currently) has the form
// `{ strong, weak, data }`.
struct ArcTask<Fut>(PhantomData<Fut>);

struct ArcTaskUnowned<Fut>(PhantomData<Fut>); // Doesn't drop the `Arc`'s data

// We should never touch the future `Fut` on any thread other than the one
// owning `FuturesUnordered`, so this should be a safe operation.
unsafe impl<Fut> Send for ArcTask<Fut> {}
unsafe impl<Fut> Sync for ArcTask<Fut> {}

unsafe impl<Fut> Send for ArcTaskUnowned<Fut> {}
unsafe impl<Fut> Sync for ArcTaskUnowned<Fut> {}

// We need to implement `UnsafeWake` trait directly and can't implement `Wake`
// for `Task<Fut>` because `Fut`, the future, isn't required to have a static
// lifetime. `UnsafeWake` lets us forget about `Fut` and its lifetime. This is
// safe because neither `drop_raw` nor `wake` touch `Fut`. This is the case even
// though `drop_raw` runs the destructor for `Task<Fut>` because its destructor
// is guaranteed to not touch `Fut`. `Fut` must already have been dropped by the
// time it runs. See `Drop` impl for `Task<Fut>` for more details.
unsafe impl<Fut> UnsafeWake for ArcTask<Fut> {
    #[inline]
    unsafe fn clone_raw(&self) -> Waker {
        let me: *const ArcTask<Fut> = self;
        let task = &*(&me as *const *const ArcTask<Fut>
                          as *const Arc<Task<Fut>>);
        Task::waker(task)
    }

    #[inline]
    unsafe fn drop_raw(&self) {
        let mut me: *const ArcTask<Fut> = self;
        let task_ptr = &mut me as *mut *const ArcTask<Fut>
                               as *mut Arc<Task<Fut>>;
        ptr::drop_in_place(task_ptr);
    }

    #[inline]
    unsafe fn wake(&self) {
        let me: *const ArcTask<Fut> = self;
        let task = &*(&me as *const *const ArcTask<Fut>
                          as *const Arc<Task<Fut>>);
        Task::wake(task);
    }
}

unsafe impl<Fut> UnsafeWake for ArcTaskUnowned<Fut> {
    #[inline]
    unsafe fn clone_raw(&self) -> Waker {
        let me: *const ArcTaskUnowned<Fut> = self;
        let task = &*(&me as *const *const ArcTaskUnowned<Fut>
                          as *const Arc<Task<Fut>>);
        // Clones the `Arc` and the returned waker owns the
        // clone. (`ArcTask<Fut>` not `ArcTaskUnowned<Fut>`)
        Task::waker(task)
    }

    #[inline]
    unsafe fn drop_raw(&self) {} // Does nothing

    #[inline]
    unsafe fn wake(&self) {
        let me: *const ArcTaskUnowned<Fut> = self;
        let task = &*(&me as *const *const ArcTaskUnowned<Fut>
                          as *const Arc<Task<Fut>>);
       Task::wake(task);
    }
}
