use std::cell::UnsafeCell;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{Relaxed, Acquire, Release, AcqRel};

use futures_core::task::AtomicWaker;

use super::abort::abort;
use super::node::Node;

pub(super) enum Dequeue<T> {
    Data(*const Node<T>),
    Empty,
    Inconsistent,
}

pub(super) struct Inner<T> {
    // The task using `FuturesUnordered`.
    pub(super) parent: AtomicWaker,

    // Head/tail of the readiness queue
    pub(super) head_readiness: AtomicPtr<Node<T>>,
    pub(super) tail_readiness: UnsafeCell<*const Node<T>>,
    pub(super) stub: Arc<Node<T>>,
}

impl<T> Inner<T> {
    /// The enqueue function from the 1024cores intrusive MPSC queue algorithm.
    pub(super) fn enqueue(&self, node: *const Node<T>) {
        unsafe {
            debug_assert!((*node).queued.load(Relaxed));

            // This action does not require any coordination
            (*node).next_readiness.store(ptr::null_mut(), Relaxed);

            // Note that these atomic orderings come from 1024cores
            let node = node as *mut _;
            let prev = self.head_readiness.swap(node, AcqRel);
            (*prev).next_readiness.store(node, Release);
        }
    }

    /// The dequeue function from the 1024cores intrusive MPSC queue algorithm
    ///
    /// Note that this is unsafe as it required mutual exclusion (only one
    /// thread can call this) to be guaranteed elsewhere.
    pub(super) unsafe fn dequeue(&self) -> Dequeue<T> {
        let mut tail = *self.tail_readiness.get();
        let mut next = (*tail).next_readiness.load(Acquire);

        if tail == self.stub() {
            if next.is_null() {
                return Dequeue::Empty;
            }

            *self.tail_readiness.get() = next;
            tail = next;
            next = (*next).next_readiness.load(Acquire);
        }

        if !next.is_null() {
            *self.tail_readiness.get() = next;
            debug_assert!(tail != self.stub());
            return Dequeue::Data(tail);
        }

        if self.head_readiness.load(Acquire) as *const _ != tail {
            return Dequeue::Inconsistent;
        }

        self.enqueue(self.stub());

        next = (*tail).next_readiness.load(Acquire);

        if !next.is_null() {
            *self.tail_readiness.get() = next;
            return Dequeue::Data(tail);
        }

        Dequeue::Inconsistent
    }

    pub(super) fn stub(&self) -> *const Node<T> {
        &*self.stub
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        // Once we're in the destructor for `Inner<T>` we need to clear out the
        // mpsc queue of nodes if there's anything left in there.
        //
        // Note that each node has a strong reference count associated with it
        // which is owned by the mpsc queue. All nodes should have had their
        // futures dropped already by the `FuturesUnordered` destructor above,
        // so we're just pulling out nodes and dropping their refcounts.
        unsafe {
            loop {
                match self.dequeue() {
                    Dequeue::Empty => break,
                    Dequeue::Inconsistent => abort("inconsistent in drop"),
                    Dequeue::Data(ptr) => drop(Arc::from_raw(ptr)),
                }
            }
        }
    }
}
