use std::cell::UnsafeCell;
use std::mem;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicPtr, AtomicBool};
use std::sync::atomic::Ordering::SeqCst;

use futures_core::task::{UnsafeWake, Waker, LocalWaker};

use super::ReadyToRunQueue;
use super::abort::abort;

pub(super) struct Node<T> {
    // The future
    pub(super) future: UnsafeCell<Option<T>>,

    // Next pointer for linked list tracking all active nodes
    pub(super) next_all: UnsafeCell<*const Node<T>>,

    // Previous node in linked list tracking all active nodes
    pub(super) prev_all: UnsafeCell<*const Node<T>>,

    // Next pointer in readiness queue
    pub(super) next_ready_to_run: AtomicPtr<Node<T>>,

    // Queue that we'll be enqueued to when notified
    pub(super) ready_to_run_queue: Weak<ReadyToRunQueue<T>>,

    // Whether or not this node is currently in the mpsc queue.
    pub(super) queued: AtomicBool,
}

impl<T> Node<T> {
    pub(super) fn wake(self: &Arc<Node<T>>) {
        let inner = match self.ready_to_run_queue.upgrade() {
            Some(inner) => inner,
            None => return,
        };

        // It's our job to notify the node that it's ready to get polled,
        // meaning that we need to enqueue it into the readiness queue. To
        // do this we flag that we're ready to be queued, and if successful
        // we then do the literal queueing operation, ensuring that we're
        // only queued once.
        //
        // Once the node is inserted we be sure to notify the parent task,
        // as it'll want to come along and pick up our node now.
        //
        // Note that we don't change the reference count of the node here,
        // we're just enqueueing the raw pointer. The `FuturesUnordered`
        // implementation guarantees that if we set the `queued` flag true that
        // there's a reference count held by the main `FuturesUnordered` queue
        // still.
        let prev = self.queued.swap(true, SeqCst);
        if !prev {
            inner.enqueue(&**self);
            inner.parent.wake();
        }
    }

    // Saftey: The returned `NonNull<Unsafe>` needs to be put into a `Waker`
    // or `LocalWaker`
    unsafe fn clone_as_unsafe_wake_without_lifetime(self: &Arc<Node<T>>)
        -> NonNull<UnsafeWake>
    {
        let clone = self.clone();

        // Safety: This is save because an `Arc` is a struct which contains
        // a single field that is a pointer.
        let ptr = mem::transmute::<Arc<Node<T>>, NonNull<ArcNode<T>>>(clone);

        let ptr = ptr as NonNull<UnsafeWake>;

        // Hide lifetime of `T`
        // Safety: This is safe because `UnsafeWake` is guaranteed not to
        // touch `T`
        mem::transmute::<NonNull<UnsafeWake>, NonNull<UnsafeWake>>(ptr)
    }

    pub(super) fn local_waker(self: &Arc<Node<T>>) -> LocalWaker {
        unsafe { LocalWaker::new(self.clone_as_unsafe_wake_without_lifetime()) }
    }

    pub(super) fn waker(self: &Arc<Node<T>>) -> Waker {
        unsafe { Waker::new(self.clone_as_unsafe_wake_without_lifetime()) }
    }
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        // Currently a `Node<T>` is sent across all threads for any lifetime,
        // regardless of `T`. This means that for memory safety we can't
        // actually touch `T` at any time except when we have a reference to the
        // `FuturesUnordered` itself.
        //
        // Consequently it *should* be the case that we always drop futures from
        // the `FuturesUnordered` instance, but this is a bomb in place to catch
        // any bugs in that logic.
        unsafe {
            if (*self.future.get()).is_some() {
                abort("future still here when dropping");
            }
        }
    }
}

// `ArcNode<T>` represents conceptually the struct an `Arc<Node<T>>` points to.
// `*const ArcNode<T>` is equal to `Arc<Node<T>>`
// It may only be used through references because its layout obviously doesn't
// match the real inner struct of an `Arc` which (currently) has the form
// `{ strong, weak, data }`.
struct ArcNode<T>(PhantomData<T>);

// We should never touch the future `T` on any thread other than the one owning
// `FuturesUnordered`, so this should be a safe operation.
unsafe impl<T> Send for ArcNode<T> {}
unsafe impl<T> Sync for ArcNode<T> {}

// We need to implement `UnsafeWake` trait directly and can't implement `Wake`
// for `Node<T>` because `T`, the future, isn't required to have a static
// lifetime. `UnsafeWake` lets us forget about `T` and its lifetime. This is
// safe because neither `drop_raw` nor `wake` touch `T`. This is the case even
// though `drop_raw` runs the destructor for `Node<T>` because its destructor is
// guaranteed to not touch `T`. `T` must already have been dropped by the time
// it runs. See `Drop` impl for `Node<T>` for more details.
unsafe impl<T> UnsafeWake for ArcNode<T> {
    #[inline]
    unsafe fn clone_raw(&self) -> Waker {
        let me: *const ArcNode<T> = self;
        let node = &*(&me as *const *const ArcNode<T> as *const Arc<Node<T>>);
        Node::waker(node)
    }

    #[inline]
    unsafe fn drop_raw(&self) {
        let mut me: *const ArcNode<T> = self;
        let node_ptr = &mut me as *mut *const ArcNode<T> as *mut Arc<Node<T>>;
        ptr::drop_in_place(node_ptr);
    }

    #[inline]
    unsafe fn wake(&self) {
        let me: *const ArcNode<T> = self;
        let node = &*(&me as *const *const ArcNode<T> as *const Arc<Node<T>>);
        Node::wake(node)
    }
}
