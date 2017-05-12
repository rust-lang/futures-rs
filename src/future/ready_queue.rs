use {task, Stream, Future, Poll, Async};
use executor::{Notify};
use task_impl::{self, AtomicTask};

use std::{mem, ptr, usize};
use std::boxed::Box;
use std::cell::UnsafeCell;
use std::fmt::{self, Debug};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicPtr};
use std::sync::atomic::Ordering::{Relaxed, AcqRel, Acquire, Release};

/// An unbounded queue of futures.
///
/// Futures are pushed into the queue and their realized values are yielded as
/// they are ready.
pub struct ReadyQueue<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    // Stub node
    stub: Box<Node<T>>,

    // The task using `ReadyQueue`.
    parent: AtomicTask,

    // Head of the readiness queue
    head: AtomicPtr<Node<T>>,

    // Tail of readiness queue
    tail: UnsafeCell<*mut Node<T>>,
}

struct Node<T> {
    // The future
    future: UnsafeCell<Option<T>>,

    // Next pointer
    next: AtomicPtr<Node<T>>,

    // Atomic state, includes the ref count
    state: AtomicUsize,
}

enum Dequeue<T> {
    Data(*mut Node<T>),
    Empty,
    Inconsistent,
}

/// Max number of references to a single node
const MAX_REFS: usize = usize::MAX >> 1;

/// Flag tracking that a node has been queued.
const QUEUED: usize = usize::MAX - (usize::MAX >> 1);

impl<T> ReadyQueue<T>
    where T: Future + 'static,
{

    /// Constructs a new, empty `ReadyQueue`
    pub fn new() -> ReadyQueue<T> {
        let mut stub = Box::new(Node {
            future: UnsafeCell::new(None),
            next: AtomicPtr::new(ptr::null_mut()),
            state: AtomicUsize::new(QUEUED | 1),
        });

        debug_assert!(stub.state.load(Relaxed) & QUEUED == QUEUED);

        ReadyQueue {
            inner: Arc::new(Inner {
                parent: AtomicTask::new(),
                head: AtomicPtr::new(&mut *stub as *mut _),
                tail: UnsafeCell::new(&mut *stub as *mut _),
                stub: stub,
            }),
        }
    }

    /// Push a future into the queue.
    ///
    /// **IMPORTANT** You *must* call `poll` after pushing futures onto the
    /// queue.
    pub fn push(&mut self, future: T) {
        let node = Box::new(Node {
            future: UnsafeCell::new(Some(future)),
            next: AtomicPtr::new(ptr::null_mut()),
            state: AtomicUsize::new(QUEUED | 1),
        });

        let ptr = Box::into_raw(node);

        // Enqueue the node
        self.inner.enqueue(ptr);
    }
}

impl<T> Stream for ReadyQueue<T>
    where T: Future + 'static
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Option<T::Item>, T::Error> {
        // Ensure `parent` is correctly set
        unsafe { self.inner.parent.park() };

        loop {
            match unsafe { self.inner.dequeue() } {
                Dequeue::Empty => return Ok(Async::NotReady),
                Dequeue::Inconsistent => {
                    // At this point, it may be worth yielding the thread &
                    // spinning a few times... but for now, just yield using the
                    // task system.
                    task::current().notify();
                    return Ok(Async::NotReady);
                }
                Dequeue::Data(node) => {
                    debug_assert!(node != self.inner.stub());
                    let node = unsafe { &mut *node };

                    // Unset queued flag
                    node.state.fetch_and(!QUEUED, AcqRel);

                    // Only try running the future if it hasn't already been
                    // completed.
                    match unsafe { &mut *node.future.get() } {
                        &mut Some(ref mut f) => {
                            let notify = self.inner.clone().into();
                            let id = node as *const _ as u64;

                            // Poll the future
                            let res = task_impl::with_notify(&notify, id, || {
                                f.poll()
                            });

                            // Release the node handle
                            unsafe { release(node) };

                            match res {
                                Ok(Async::NotReady) => {}
                                Ok(Async::Ready(v)) => {
                                    return Ok(Async::Ready(Some(v)))
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        &mut None => {
                            // Release the node
                            unsafe { release(node) };
                        }
                    }
                }
            }
        }
    }
}

impl<T: Debug> Debug for ReadyQueue<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "ReadyQueue {{ ... }}")
    }
}

impl<T> Inner<T> where T: Future {
    /// The enqueue function from the 1024cores intrusive MPSC queue algorithm.
    fn enqueue(&self, node: *mut Node<T>) {
        unsafe {
            debug_assert!((*node).state.load(Relaxed) & QUEUED == QUEUED);

            // This action does not require any coordination
            (*node).next.store(ptr::null_mut(), Relaxed);

            let prev = self.head.swap(node, AcqRel);
            (*prev).next.store(node, Release);
        }
    }

    /// The dequeue function from the 1024cores intrusive MPSC queue algorithm
    unsafe fn dequeue(&self) -> Dequeue<T> {
        // This is the 1024cores.net intrusive MPSC queue [1] "pop" function
        // with the modifications mentioned at the top of the file.
        let mut tail = *self.tail.get();
        let mut next = (*tail).next.load(Acquire);

        if tail == self.stub() {
            if next.is_null() {
                return Dequeue::Empty;
            }

            *self.tail.get() = next;
            tail = next;
            next = (*next).next.load(Acquire);
        }

        if !next.is_null() {
            *self.tail.get() = next;
            debug_assert!(tail != self.stub());
            return Dequeue::Data(tail);
        }

        if self.head.load(Acquire) != tail {
            return Dequeue::Inconsistent;
        }

        // Push the stub node
        debug_assert!(self.stub.state.load(Relaxed) & QUEUED == QUEUED);
        self.enqueue(self.stub());

        next = (*tail).next.load(Acquire);

        if !next.is_null() {
            *self.tail.get() = next;
            debug_assert!(tail != self.stub());
            return Dequeue::Data(tail);
        }

        Dequeue::Inconsistent
    }

    fn stub(&self) -> *mut Node<T> {
        let ret = &*self.stub as *const _ as *mut _;
        debug_assert!(self.stub.state.load(Relaxed) & QUEUED == QUEUED);
        ret
    }
}

impl<T> Notify for Inner<T> where T: Future {
    fn notify(&self, id: u64) {
        unsafe {
            let node: &Node<T> = Node::from_id(id);

            debug_assert!(node as *const _ as *mut _ != self.stub());
            let mut curr = node.state.load(Acquire);

            loop {
                if curr & QUEUED == QUEUED {
                    // Nothing more to do
                    return;
                }

                // TODO: Prevent overflow
                let next = (curr | QUEUED) + 1;

                let act = node.state.compare_and_swap(curr, next, AcqRel);

                if curr == act {
                    // Enqueue the task
                    self.enqueue(node as *const _ as *mut _);

                    // Notify the parent after the task has been enqueued
                    self.parent.notify();

                    return;
                }

                curr = act;
            }
        }
    }

    fn ref_inc(&self, id: u64) {
        unsafe {
            let node: &Node<T> = Node::from_id(id);

            // Using a relaxed ordering is alright here, as knowledge of the
            // original reference prevents other threads from erroneously
            // deleting the object.
            //
            // As explained in the [Boost documentation][1], Increasing the
            // reference counter can always be done with memory_order_relaxed:
            // New references to an object can only be formed from an existing
            // reference, and passing an existing reference from one thread to
            // another must already provide any required synchronization.
            //
            // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
            debug_assert!(node as *const _ as *mut _ != self.stub());
            let old_size = node.state.fetch_add(1, Relaxed);

            if old_size > MAX_REFS {
                panic!(); // TODO: abort
            }
        }
    }

    fn ref_dec(&self, id: u64) {
        unsafe {
            let node: &Node<T> = Node::from_id(id);
            debug_assert!(node as *const _ as *mut _ != self.stub());
            release(node);
        }
    }
}

unsafe impl<T> Send for Inner<T> {}
unsafe impl<T> Sync for Inner<T> {}

impl<T> Node<T> {
    unsafe fn from_id<'a>(id: u64) -> &'a Node<T> {
        mem::transmute(id as usize)
    }
}

unsafe fn release<T>(node: &Node<T>) {
    let old_state = node.state.fetch_sub(1, AcqRel);

    if old_state != 1 {
        return;
    }

    let _: Box<Node<T>> = Box::from_raw(node as *const _ as *mut _);
}
