#![allow(dead_code)]

use super::Task;

use core::fmt;
use core::cell::UnsafeCell;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{Acquire, Release};

/// A coordinated `Task` handle enabling concurrent operations on a task.
pub struct AtomicTask {
    state: AtomicUsize,
    task: UnsafeCell<Option<Task>>,
}

/// Initial state, the `AtomicTask` is currently not being used.
///
/// The value `2` is picked specifically because it between the write lock &
/// read lock values. Since the read lock is represented by an incrementing
/// counter, this enables an atomic fetch_sub operation to be used for releasing
/// a lock.
const WAITING: usize = 2;

/// The `park` function has determined that the task is no longer current. This
/// implies that `AtomicTask::park` is being called from a different task than
/// is represented by the currently stored task. The write lock is obtained to
/// update the task cell.
const LOCKED_WRITE: usize = 0;

/// At least one call to `notify` happened concurrently to `park` updating the
/// task cell. This state is detected when `park` exits the mutation code and
/// signals to `park` that it is responsible for notifying its own task.
const LOCKED_WRITE_NOTIFIED: usize = 1;


/// The `notify` function has locked access to the task cell for notification.
///
/// The constant is left here mostly for documentation reasons.
#[allow(dead_code)]
const LOCKED_READ: usize = 3;

impl AtomicTask {
    /// Create an `AtomicTask` initialized with the given `Task`
    pub fn new() -> AtomicTask {
        // Make sure that task is Sync
        fn is_sync<T: Sync>() {}
        is_sync::<Task>();

        AtomicTask {
            state: AtomicUsize::new(WAITING),
            task: UnsafeCell::new(None),
        }
    }

    /// The caller must ensure mutual exclusion
    pub unsafe fn park(&self) {
        if let Some(ref task) = *self.task.get() {
            if task.will_notify_current() {
                // Nothing more to do
                return
            }
        }

        // Get a new task handle
        let task = super::current();

        match self.state.compare_and_swap(WAITING, LOCKED_WRITE, Acquire) {
            WAITING => {
                // Locked acquired, update the task cell
                *self.task.get() = Some(task);

                // Release the lock. If the state transitioned to
                // `LOCKED_NOTIFIED`, this means that an notify has been
                // signaled, so notify the task.
                if LOCKED_WRITE_NOTIFIED == self.state.swap(WAITING, Release) {
                    (*self.task.get()).as_ref().unwrap().notify();
                }
            }
            state => {
                debug_assert!(state != LOCKED_WRITE, "unexpected state LOCKED_WRITE");
                debug_assert!(state != LOCKED_WRITE_NOTIFIED, "unexpected state LOCKED_WRITE_NOTIFIED");

                // Currently in a read locked state, this implies that `notify`
                // is currently being called on the old task handle. So, we call
                // notify on the new task handle
                task.notify();
            }
        }
    }

    pub fn notify(&self) {
        let mut curr = WAITING;

        loop {
            if curr == LOCKED_WRITE {
                // Transition the state to LOCKED_NOTIFIED
                let actual = self.state.compare_and_swap(LOCKED_WRITE, LOCKED_WRITE_NOTIFIED, Release);

                if curr == actual {
                    // Success, return
                    return;
                }

                // update current state variable and try again
                curr = actual;

            } else if curr == LOCKED_WRITE_NOTIFIED {
                // Currently in `LOCKED_WRITE_NOTIFIED` state, nothing else to do.
                return;

            } else {
                // Currently in a LOCKED_READ state, so attempt to increment the
                // lock count.
                let actual = self.state.compare_and_swap(curr, curr + 1, Acquire);

                // Locked acquired
                if actual == curr {
                    // Notify the task
                    unsafe {
                        if let Some(ref task) = *self.task.get() {
                            task.notify();
                        }
                    }

                    // Release the lock
                    self.state.fetch_sub(1, Release);

                    // Done
                    return;
                }

                // update current state variable and try again
                curr = actual;

            }
        }
    }
}

impl fmt::Debug for AtomicTask {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "AtomicTask")
    }
}

unsafe impl Send for AtomicTask {}
unsafe impl Sync for AtomicTask {}
