use crate::task::AtomicWaker;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::AtomicPtr;
use std::sync::atomic::Ordering::{Relaxed, Acquire, Release, AcqRel};

use super::abort::abort;
use super::node::Node;

pub(super) enum Dequeue<Fut> {
    Data(*const Node<Fut>),
    Empty,
    Inconsistent,
}

pub(super) struct ReadyToRunQueue<Fut> {
    // The task using `FuturesUnordered`.
    pub(super) parent: AtomicWaker,

    // Head/tail of the readiness queue
    pub(super) head: AtomicPtr<Node<Fut>>,
    pub(super) tail: UnsafeCell<*const Node<Fut>>,
    pub(super) stub: Arc<Node<Fut>>,
}

/// An MPSC queue into which the nodes containing the futures are inserted
/// whenever the future inside is scheduled for polling.
impl<Fut> ReadyToRunQueue<Fut> {
    /// The enqueue function from the 1024cores intrusive MPSC queue algorithm.
    pub(super) fn enqueue(&self, node: *const Node<Fut>) {
        unsafe {
            debug_assert!((*node).queued.load(Relaxed));

            // This action does not require any coordination
            (*node).next_ready_to_run.store(ptr::null_mut(), Relaxed);

            // Note that these atomic orderings come from 1024cores
            let node = node as *mut _;
            let prev = self.head.swap(node, AcqRel);
            (*prev).next_ready_to_run.store(node, Release);
        }
    }

    /// The dequeue function from the 1024cores intrusive MPSC queue algorithm
    ///
    /// Note that this is unsafe as it required mutual exclusion (only one
    /// thread can call this) to be guaranteed elsewhere.
    pub(super) unsafe fn dequeue(&self) -> Dequeue<Fut> {
        let mut tail = *self.tail.get();
        let mut next = (*tail).next_ready_to_run.load(Acquire);

        if tail == self.stub() {
            if next.is_null() {
                return Dequeue::Empty;
            }

            *self.tail.get() = next;
            tail = next;
            next = (*next).next_ready_to_run.load(Acquire);
        }

        if !next.is_null() {
            *self.tail.get() = next;
            debug_assert!(tail != self.stub());
            return Dequeue::Data(tail);
        }

        if self.head.load(Acquire) as *const _ != tail {
            return Dequeue::Inconsistent;
        }

        self.enqueue(self.stub());

        next = (*tail).next_ready_to_run.load(Acquire);

        if !next.is_null() {
            *self.tail.get() = next;
            return Dequeue::Data(tail);
        }

        Dequeue::Inconsistent
    }

    pub(super) fn stub(&self) -> *const Node<Fut> {
        &*self.stub
    }
}

impl<Fut> Drop for ReadyToRunQueue<Fut> {
    fn drop(&mut self) {
        // Once we're in the destructor for `Inner<Fut>` we need to clear out
        // the ready to run queue of nodes if there's anything left in there.
        //
        // Note that each node has a strong reference count associated with it
        // which is owned by the ready to run queue. All nodes should have had
        // their futures dropped already by the `FuturesUnordered` destructor
        // above, so we're just pulling out nodes and dropping their refcounts.
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
