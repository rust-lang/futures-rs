//! An asynchronously awaitable event for signalization between tasks

use futures_core::future::{Future, FusedFuture};
use futures_core::task::{LocalWaker, Poll, Waker};
use core::pin::Pin;
use core::cell::RefCell;
use crate::intrusive_list::{LinkedList, ListNode};

/// Tracks how the future had interacted with the event
#[derive(PartialEq)]
enum PollState {
    /// The task has never interacted with the event.
    New,
    /// The task was added to the wait queue at the event.
    Waiting,
    /// The task had been polled to completion.
    Done,
}

/// Tracks the WaitForEventFuture waiting state.
/// Access to this struct is synchronized through the mutex in the Event.
struct WaitQueueEntry {
    /// The task handle of the waiting task
    task: Option<Waker>,
    /// Current polling state
    state: PollState,
}

impl WaitQueueEntry {
    /// Creates a new WaitQueueEntry
    fn new() -> WaitQueueEntry {
        WaitQueueEntry {
            task: None,
            state: PollState::New,
        }
    }
}

/// Internal state of the `ManualResetEvent` pair above
struct EventState {
    is_set: bool,
    waiters: LinkedList<WaitQueueEntry>,
}

impl EventState {
    fn new(is_set: bool) -> EventState {
        EventState {
            is_set,
            waiters: LinkedList::new(),
        }
    }

    fn reset(&mut self) {
        self.is_set = false;
    }

    fn set(&mut self) {
        if self.is_set != true {
            self.is_set = true;

            // Wakeup all waiters
            // This happens inside the lock to make cancellation reliable
            // If we would access waiters outside of the lock, the pointers
            // may no longer be valid.
            // Typically this shouldn't be an issue, since waking a task should
            // only move it from the blocked into the ready state and not have
            // further side effects.

            let mut waiters = self.waiters.take();

            unsafe {
                // Reverse the waiter list, so that the oldest waker (which is
                // at the end of the list), gets woken first.
                waiters.reverse();

                for waiter in waiters.into_iter() {
                    let task = (*waiter).task.take();
                    if let Some(ref handle) = task {
                        handle.wake();
                    }
                    (*waiter).state = PollState::Done;
                }
            }
        }
    }

    fn is_set(&self) -> bool {
        self.is_set
    }

    /// Checks if the event is set. If it is this returns immediately.
    /// If the event isn't set, the WaitForEventFuture gets added to the wait
    /// queue at the event, and will be signalled once ready.
    /// This function is only safe as long as the `wait_node`s address is guaranteed
    /// to be stable until it gets removed from the queue.
    unsafe fn try_wait(
        &mut self,
        wait_node: &mut ListNode<WaitQueueEntry>,
        lw: &LocalWaker,
    ) -> Poll<()> {
        match wait_node.state {
            PollState::New => {
                if self.is_set {
                    // The event is already signaled
                    wait_node.state = PollState::Done;
                    Poll::Ready(())
                }
                else {
                    // Added the task to the wait queue
                    wait_node.task = Some(lw.clone().into_waker());
                    wait_node.state = PollState::Waiting;
                    self.waiters.add_front(wait_node);
                    Poll::Pending
                }
            },
            PollState::Waiting => {
                // The WaitForEventFuture is already in the queue.
                // The event can't have been set, since this would change the
                // waitstate inside the mutex.
                Poll::Pending
            },
            PollState::Done => {
                // We had been woken up by the event.
                // This does not guarantee that the event is still set. It could
                // have been reset it in the meantime.
                Poll::Ready(())
            },
        }
    }

    fn remove_waiter(&mut self, wait_node: &mut ListNode<WaitQueueEntry>) {
        // WaitForEventFuture only needs to get removed if it had been added to
        // the wait queue of the Event. This has happened in the PollState::Waiting case.
        if let PollState::Waiting = wait_node.state {
            if ! unsafe { self.waiters.remove(wait_node) } {
                // Panic if the address isn't found. This can only happen if the contract was
                // violated, e.g. the WaitQueueEntry got moved after the initial poll.
                panic!("Future could not be removed from wait queue");
            }
            wait_node.state = PollState::Done;
        }
    }
}

/// A synchronization primitive which can be either in the set or reset state.
///
/// Tasks can wait for the event to get set by obtaining a Future via `wait`.
/// This Future will get fulfilled when the event had been set.
pub struct LocalManualResetEvent {
    inner: RefCell<EventState>,
}

// The Event is can be sent to other threads as long as it's not borrowed
unsafe impl Send for LocalManualResetEvent {}

impl core::fmt::Debug for LocalManualResetEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalManualResetEvent")
            .finish()
    }
}

impl LocalManualResetEvent {
    /// Creates a new LocalManualResetEvent in the given state
    pub fn new(is_set: bool) -> LocalManualResetEvent {
        LocalManualResetEvent {
            inner: RefCell::new(EventState::new(is_set)),
        }
    }

    /// Sets the event.
    ///
    /// Setting the event will notify all pending waiters.
    pub fn set(&self) {
        self.inner.borrow_mut().set()
    }

    /// Resets the event.
    pub fn reset(&self) {
        self.inner.borrow_mut().reset()
    }

    /// Returns whether the event is set
    pub fn is_set(&self) -> bool {
        self.inner.borrow().is_set()
    }

    /// Returns a future that gets fulfilled when the event is set.
    pub fn wait<'a>(&'a self) -> LocalWaitForEventFuture {
        LocalWaitForEventFuture {
            event: Some(&self),
            wait_node: ListNode::new(WaitQueueEntry::new()),
        }
    }
}

/// A Future that is resolved once the corresponding LocalManualResetEvent has been set
#[must_use = "futures do nothing unless polled"]
pub struct LocalWaitForEventFuture<'a> {
    /// The LocalManualResetEvent that is associated with this WaitForEventFuture
    event: Option<&'a LocalManualResetEvent>,
    /// Node for waiting at the event
    wait_node: ListNode<WaitQueueEntry>,
}

impl<'a> core::fmt::Debug for LocalWaitForEventFuture<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalWaitForEventFuture")
            .finish()
    }
}

impl<'a> Future for LocalWaitForEventFuture<'a> {
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<()> {
        // It might be possible to use Pin::map_unchecked here instead of the two unsafe APIs.
        // However this didn't seem to work for some borrow checker reasons

        // Safety: The next operations are safe, because Pin promises us that
        // the address of the wait queue entry inside MutexLocalFuture is stable,
        // and we don't move any fields inside the future until it gets dropped.
        let mut_self: &mut LocalWaitForEventFuture = unsafe {
            
            Pin::get_unchecked_mut(self)
        };

        let event = mut_self.event.expect("polled LocalWaitForEventFuture after completion");

        let poll_res = unsafe {
            event.inner.borrow_mut().try_wait(
                &mut mut_self.wait_node,
                lw)
        };

        if let Poll::Ready(()) = poll_res {
            // The event was set
            mut_self.event = None;
        }

        poll_res
    }
}

impl<'a> FusedFuture for LocalWaitForEventFuture<'a> {
    fn is_terminated(&self) -> bool {
        self.event.is_none()
    }
}

impl<'a> Drop for LocalWaitForEventFuture<'a> {
    fn drop(&mut self) {
        // If this WaitForEventFuture has been polled and it was added to the
        // wait queue at the event, it must be removed before dropping.
        // Otherwise the event would access invalid memory.
        if let Some(ev) = self.event {
            ev.inner.borrow_mut().remove_waiter(&mut self.wait_node);
        }
    }
}

#[cfg(feature = "std")]
mod if_std {
    use super::*;
    use std::sync::Mutex;

    /// A synchronization primitive which can be either in the set or reset state.
    ///
    /// Tasks can wait for the event to get set by obtaining a Future via `wait`.
    /// This Future will get fulfilled when the event had been set.
    pub struct ManualResetEvent {
        inner: Mutex<EventState>,
    }

    // The Event is thread-safe and can be sent to other threads.
    // Automatic derive doesn't work due to the unsafe pointer in WaitQueueEntry
    unsafe impl Send for ManualResetEvent {}
    unsafe impl Sync for ManualResetEvent {}

    impl core::fmt::Debug for ManualResetEvent {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("ManualResetEvent")
                .finish()
        }
    }

    impl ManualResetEvent {
        /// Creates a new ManualResetEvent in the given state
        pub fn new(is_set: bool) -> ManualResetEvent {
            ManualResetEvent {
                inner: Mutex::new(EventState::new(is_set)),
            }
        }

        /// Sets the event.
        ///
        /// Setting the event will notify all pending waiters.
        pub fn set(&self) {
            let mut ev_state = self.inner.lock().unwrap();
            ev_state.set()
        }

        /// Resets the event.
        pub fn reset(&self) {
            let mut ev_state = self.inner.lock().unwrap();
            ev_state.reset()
        }

        /// Returns whether the event is set
        pub fn is_set(&self) -> bool {
            let ev_state = self.inner.lock().unwrap();
            ev_state.is_set()
        }

        /// Returns a future that gets fulfilled when the event is set.
        pub fn wait(&self) -> WaitForEventFuture {
            WaitForEventFuture {
                event: Some(&self),
                wait_node: ListNode::new(WaitQueueEntry::new()),
            }
        }
    }

    /// A Future that is resolved once the corresponding ManualResetEvent has been set
    #[must_use = "futures do nothing unless polled"]
    pub struct WaitForEventFuture<'a> {
        /// The ManualResetEvent this is associated with this WaitForEventFuture
        event: Option<&'a ManualResetEvent>,
        /// Node for waiting at the event
        wait_node: ListNode<WaitQueueEntry>,
    }

    impl<'a> core::fmt::Debug for WaitForEventFuture<'a> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("WaitForEventFuture")
                .finish()
        }
    }

    impl<'a> Future for WaitForEventFuture<'a> {
        type Output = ();

        fn poll(
            self: Pin<&mut Self>,
            lw: &LocalWaker,
        ) -> Poll<()> {
            // Safety: The next operations are safe, because Pin promises us that
            // the address of the wait queue entry inside MutexLocalFuture is stable,
            // and we don't move any fields inside the future until it gets dropped.
            let mut_self: &mut WaitForEventFuture = unsafe { Pin::get_unchecked_mut(self) };
            
            let event = mut_self.event.expect("polled WaitForEventFuture after completion");

            let poll_res = unsafe {
                event.inner.lock().unwrap().try_wait(
                    &mut mut_self.wait_node,
                    lw)
            };

            if let Poll::Ready(()) = poll_res {
                // The event was set
                mut_self.event = None;
            }

            poll_res
        }
    }

    impl<'a> FusedFuture for WaitForEventFuture<'a> {
        fn is_terminated(&self) -> bool {
            self.event.is_none()
        }
    }

    impl<'a> Drop for WaitForEventFuture<'a> {
        fn drop(&mut self) {
            // If this WaitForEventFuture has been polled and it was added to the
            // wait queue at the event, it must be removed before dropping.
            // Otherwise the event would access invalid memory.
            if let Some(ev) = self.event {
                ev.inner.lock().unwrap().remove_waiter(&mut self.wait_node);
            }
        }
    }
}

#[cfg(feature = "std")]
pub use self::if_std::*;