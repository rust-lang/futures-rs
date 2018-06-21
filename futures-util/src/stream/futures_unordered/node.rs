use std::cell::UnsafeCell;
use std::mem;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicPtr, AtomicBool};
use std::sync::atomic::Ordering::SeqCst;

use futures_core::task::{UnsafeWake, Waker, LocalWaker};

use super::Inner;
use super::abort::abort;

pub(super) struct Node<T> {
    // The future
    pub(super) future: UnsafeCell<Option<T>>,

    // Next pointer for linked list tracking all active nodes
    pub(super) next_all: UnsafeCell<*const Node<T>>,

    // Previous node in linked list tracking all active nodes
    pub(super) prev_all: UnsafeCell<*const Node<T>>,

    // Next pointer in readiness queue
    pub(super) next_readiness: AtomicPtr<Node<T>>,

    // Queue that we'll be enqueued to when notified
    pub(super) queue: Weak<Inner<T>>,

    // Whether or not this node is currently in the mpsc queue.
    pub(super) queued: AtomicBool,
}

impl<T> Node<T> {
    pub(super) fn notify(me: &Arc<Node<T>>) {
        let inner = match me.queue.upgrade() {
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
        let prev = me.queued.swap(true, SeqCst);
        if !prev {
            inner.enqueue(&**me);
            inner.parent.wake();
        }
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

#[allow(missing_debug_implementations)]
pub(super) struct NodeToHandle<'a, T: 'a>(pub(super) &'a Arc<Node<T>>);

impl<'a, T> Clone for NodeToHandle<'a, T> {
    fn clone(&self) -> Self {
        NodeToHandle(self.0)
    }
}

#[doc(hidden)]
impl<'a, T> From<NodeToHandle<'a, T>> for LocalWaker {
    fn from(handle: NodeToHandle<'a, T>) -> LocalWaker {
        unsafe {
            let ptr: Arc<Node<T>> = handle.0.clone();
            let ptr: *mut ArcNode<T> = mem::transmute(ptr);
            let ptr = mem::transmute(ptr as *mut UnsafeWake); // Hide lifetime
            LocalWaker::new(NonNull::new(ptr).unwrap())
        }
    }
}

#[doc(hidden)]
impl<'a, T> From<NodeToHandle<'a, T>> for Waker {
    fn from(handle: NodeToHandle<'a, T>) -> Waker {
        unsafe {
            let ptr: Arc<Node<T>> = handle.0.clone();
            let ptr: *mut ArcNode<T> = mem::transmute(ptr);
            let ptr = mem::transmute(ptr as *mut UnsafeWake); // Hide lifetime
            Waker::new(NonNull::new(ptr).unwrap())
        }
    }
}

struct ArcNode<T>(PhantomData<T>);

// We should never touch `T` on any thread other than the one owning
// `FuturesUnordered`, so this should be a safe operation.
unsafe impl<T> Send for ArcNode<T> {}
unsafe impl<T> Sync for ArcNode<T> {}

unsafe impl<T> UnsafeWake for ArcNode<T> {
    unsafe fn clone_raw(&self) -> Waker {
        let me: *const ArcNode<T> = self;
        let me: *const *const ArcNode<T> = &me;
        let me = &*(me as *const Arc<Node<T>>);
        NodeToHandle(me).into()
    }

    unsafe fn drop_raw(&self) {
        let mut me: *const ArcNode<T> = self;
        let me = &mut me as *mut *const ArcNode<T> as *mut Arc<Node<T>>;
        ptr::drop_in_place(me);
    }

    unsafe fn wake(&self) {
        let me: *const ArcNode<T> = self;
        let me: *const *const ArcNode<T> = &me;
        let me = me as *const Arc<Node<T>>;
        Node::notify(&*me)
    }
}
