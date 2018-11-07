//! An event for signalization between tasks

use futures_core::future::{Future, FusedFuture};
use futures_core::task::{LocalWaker, Poll, Waker};
use std::marker::{Pinned};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::ptr::null_mut;
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum PollState {
    New,
    Waiting,
    Done,
}

/// Tracks the WaitHandle registration state.
/// Access to this struct is synchronized through the mutex in the Event.
#[derive(Debug)]
struct WaitHandleRegistration {
    /// The task handle of the waiting task
    task: Option<Waker>,
    /// Next WaitHandleRegistration in the intrusive list
    next: *mut WaitHandleRegistration,
    /// Current polling state
    state: PollState,
    /// Whether WaitHandle has been polled to completion
    terminated: bool,
    /// Prevents the WaitHandleRegistration from being moved.
    /// This is important, since the address of the WaitHandleRegistration must be stable once polled.
    _pin: Pinned,
}

impl WaitHandleRegistration {
    /// Creates a new WaitHandleRegistration
    fn new() -> WaitHandleRegistration {
        WaitHandleRegistration {
            task: None,
            next: null_mut(),
            state: PollState::New,
            terminated: false,
            _pin: Pinned,
        }
    }
}

#[derive(Debug)]
struct WaitHandle {
    /// The ManualResetEvent this is associated with this WaitHandle
    event: Arc<Mutex<InnerEventState>>,
    /// Registration at the event
    reg: WaitHandleRegistration,
}

#[derive(Debug)]
struct LocalWaitHandle<'a> {
    /// The LocalManualResetEvent this is associated with this WaitHandle
    event: &'a RefCell<InnerEventState>,
    /// Registration at the event
    reg: WaitHandleRegistration,
}

/// A synchronization primitive which can be either in the set or reset state.
///
/// Tasks can wait for the event to get set by obtaining a Future via poll_set.
/// This Future will get fulfilled when the event had been set.
#[derive(Debug, Clone)]
pub struct ManualResetEvent {
    inner: Arc<Mutex<InnerEventState>>,
}

// Automatic derive doesn't work due to the unsafe pointer in WaitHandleRegistration
unsafe impl Send for ManualResetEvent {}
unsafe impl Sync for ManualResetEvent {}

/// A synchronization primitive which can be either in the set or reset state.
///
/// Tasks can wait for the event to get set by obtaining a Future via poll_set.
/// This Future will get fulfilled when the event had been set.
#[derive(Debug)]
pub struct LocalManualResetEvent {
    inner: RefCell<InnerEventState>,
}

/// Internal state of the `ManualResetEvent` pair above
#[derive(Debug)]
struct InnerEventState {
    is_set: bool,
    waiters: *mut WaitHandleRegistration,
}

impl InnerEventState {
    fn new(is_set: bool) -> InnerEventState {
        InnerEventState {
            is_set,
            waiters: null_mut(),
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

            let mut waiter = self.waiters;
            self.waiters = null_mut();

            unsafe {
                while waiter != null_mut() {
                    let task = (*waiter).task.take();
                    if let Some(ref handle) = task {
                        handle.wake();
                    }
                    (*waiter).state = PollState::Done;
                    waiter = (*waiter).next;
                }
            }
        }
    }

    fn is_set(&self) -> bool {
        self.is_set
    }

    /// Polls one WaitHandle for completion. If the event isn't set, the WaitHandle gets registered
    /// at the event, and will be signalled once ready.
    fn poll_waiter(
        &mut self,
        wait_handle: Pin<&mut WaitHandleRegistration>,
        lw: &LocalWaker,
    ) -> Poll<()> {
        let wait_handle: &mut WaitHandleRegistration = unsafe { Pin::get_mut_unchecked(wait_handle) };
        let addr = wait_handle as *mut WaitHandleRegistration;

        match wait_handle.state {
            PollState::New => {
                if self.is_set {
                    // The event is already signaled
                    wait_handle.state = PollState::Done;
                    wait_handle.terminated = true;
                    Poll::Ready(())
                }
                else {
                    // Register the WaitHandle at the event
                    wait_handle.task = Some(lw.clone().into_waker());
                    wait_handle.state = PollState::Waiting;
                    wait_handle.next = self.waiters;
                    self.waiters = addr;
                    Poll::Pending
                }
            },
            PollState::Waiting => {
                // The WaitHandle is already registered.
                // The event can't have been set, since this would change the
                // waitstate inside the mutex.
                Poll::Pending
            },
            PollState::Done => {
                // We had been woken up by the event.
                // This does not guarantee that the event is still set. It could
                // have been reset it in the meantime.
                wait_handle.terminated = true;
                Poll::Ready(())
            },
        }
    }

    fn remove_waiter(&mut self, wait_handle: &mut WaitHandleRegistration) {
        let addr = wait_handle as *mut WaitHandleRegistration;

        match wait_handle.state {
            PollState::Waiting => {
                // Remove the WaitHandle from the linked list
                if self.waiters == addr {
                    self.waiters = wait_handle.next;
                } else {
                    // Find the WaitHandle before us and link it to the one
                    // behind us
                    let mut iter = self.waiters;
                    let mut found_addr = false;

                    unsafe {
                        while iter != null_mut() {
                            if (*iter).next == addr {
                                (*iter).next = wait_handle.next;
                                found_addr = true;
                                break;
                            } else {
                                iter = (*iter).next;
                            }
                        }
                    }

                    // Panic if the address isn't found. This can only happen if the contract was
                    // violated, e.g. the WaitHandle got moved after the initial poll.
                    assert!(found_addr, "Future could not be unregistered");
                }
                wait_handle.next = null_mut();
                wait_handle.state = PollState::Done;
            },
            _ => {},
        }
    }
}

/// Creates a new LocalManualResetEvent in the given state
pub fn local_manual_reset_event(is_set: bool) -> LocalManualResetEvent {
    LocalManualResetEvent {
        inner: RefCell::new(InnerEventState::new(is_set)),
    }
}

impl<'a> LocalManualResetEvent {
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
    pub fn poll_set(&'a self) -> impl Future<Output = ()> + FusedFuture + 'a {
        LocalWaitHandle {
            event: &self.inner,
            reg: WaitHandleRegistration::new(),
        }
    }
}


/// Creates a new ManualResetEvent in the given state
pub fn manual_reset_event(is_set: bool) -> ManualResetEvent {
    let inner = Arc::new(Mutex::new(InnerEventState::new(is_set)));
    ManualResetEvent {
        inner,
    }
}

impl ManualResetEvent {
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
    pub fn poll_set(&self) -> impl Future<Output = ()> + FusedFuture {
        WaitHandle {
            event: self.inner.clone(),
            reg: WaitHandleRegistration::new(),
        }
    }
}

impl<'a> Future for LocalWaitHandle<'a> {
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<()> {
        // It might be possible to use Pin::map_unchecked here instead of the two unsafe APIs.
        // However this didn't seem to work for some borrow checker reasons
        let mut_self: &mut LocalWaitHandle = unsafe { Pin::get_mut_unchecked(self) };
        mut_self.event.borrow_mut().poll_waiter(unsafe { Pin::new_unchecked(&mut mut_self.reg) }, lw)
    }
}

impl Future for WaitHandle {
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<()> {
        let mut_self: &mut WaitHandle = unsafe { Pin::get_mut_unchecked(self) };
        let mut ev_state = mut_self.event.lock().unwrap();
        ev_state.poll_waiter(unsafe { Pin::new_unchecked(&mut mut_self.reg) }, lw)
    }
}

impl FusedFuture for WaitHandle {
    fn is_terminated(&self) -> bool {
        self.reg.terminated
    }
}

impl<'a> FusedFuture for LocalWaitHandle<'a> {
    fn is_terminated(&self) -> bool {
        self.reg.terminated
    }
}

impl Drop for WaitHandle {
    fn drop(&mut self) {
        // If this WaitHandle has been polled and it was registered at the
        // event, it must be unregistered before dropping. Otherwise the
        // event would access invalid memory.
        let mut ev_state = self.event.lock().unwrap();
        ev_state.remove_waiter(&mut self.reg);
    }
}

impl<'a> Drop for LocalWaitHandle<'a> {
    fn drop(&mut self) {
        // If this WaitHandle has been polled and it was registered at the
        // event, it must be unregistered before dropping. Otherwise the
        // event would access invalid memory.
        self.event.borrow_mut().remove_waiter(&mut self.reg);
    }
}
