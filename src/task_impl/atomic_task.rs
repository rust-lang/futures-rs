use super::Task;

use core::fmt;
use core::cell::UnsafeCell;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::{Acquire, Release, AcqRel};

/// A synchronization primitive for task notification.
///
/// `AtomicTask` will coordinate concurrent notifications with the consumer
/// potentially "updating" the underlying task to notify. This is useful in
/// scenarios where a computation completes in another thread and wants to
/// notify the consumer, but the consumer is in the process of being migrated to
/// a new logical task.
///
/// Consumers should call `register` before checking the result of a computation
/// and producers should call `notify` after producing the computation (this
/// differs from the usual `thread::park` pattern). It is also permitted for
/// `notify` to be called **before** `register`. This results in a no-op.
///
/// A single `AtomicTask` may be reused for any number of calls to `register` or
/// `notify`.
///
/// `AtomicTask` does not provide any memory ordering guarantees, as such the
/// user should use caution and use other synchronization primitives to guard
/// the result of the underlying computation.
pub struct AtomicTask {
    state: AtomicUsize,
    task: UnsafeCell<Option<Task>>,
}

/// Idle state
const WAITING: usize = 0;

/// A new task value is being registered with the `AtomicTask` cell.
const REGISTERING: usize = 0b01;

/// The task currently registered with the `AtomicTask` cell is being notified.
const NOTIFYING: usize = 0b10;

impl AtomicTask {
    /// Create an `AtomicTask` initialized with the given `Task`
    pub fn new() -> AtomicTask {
        // Make sure that task is Sync
        trait AssertSync: Sync {}
        impl AssertSync for Task {}

        AtomicTask {
            state: AtomicUsize::new(WAITING),
            task: UnsafeCell::new(None),
        }
    }

    /// Registers the current task to be notified on calls to `notify`.
    ///
    /// This is the same as calling `register_task` with `task::current()`.
    pub fn register(&self) {
        self.register_task(super::current());
    }

    /// Registers the provided task to be notified on calls to `notify`.
    ///
    /// The new task will take place of any previous tasks that were registered
    /// by previous calls to `register`. Any calls to `notify` that happen after
    /// a call to `register` (as defined by the memory ordering rules), will
    /// notify the `register` caller's task.
    ///
    /// It is safe to call `register` with multiple other threads concurrently
    /// calling `notify`. This will result in the `register` caller's current
    /// task being notified once.
    ///
    /// This function is safe to call concurrently, but this is generally a bad
    /// idea. Concurrent calls to `register` will attempt to register different
    /// tasks to be notified. One of the callers will win and have its task set,
    /// but there is no guarantee as to which caller will succeed.
    pub fn register_task(&self, task: Task) {
        match self.state.compare_and_swap(WAITING, REGISTERING, Acquire) {
            WAITING => {
                unsafe {
                    // Locked acquired, update the waker cell
                    *self.task.get() = Some(task.clone());

                    // Release the lock. If the state transitioned to include
                    // the `NOTIFYING` bit, this means that a notify has been
                    // called concurrently, so we have to remove the task and
                    // notify it.`
                    //
                    // Start by assuming that the state is `REGISTERING` as this
                    // is what we jut set it to.
                    let mut curr = REGISTERING;

                    // If a task has to be notified, it will be set here.
                    let mut notify = None;

                    loop {
                        let res = self.state.compare_exchange(
                            curr, WAITING, AcqRel, Acquire);

                        match res {
                            Ok(_) => {
                                // The atomic exchange was successful, now break
                                // out of the loop os that task stored in
                                // `notify` can be notified (if set).
                                break;
                            }
                            Err(actual) => {
                                // Update `curr` for the next iteration of the
                                // loop
                                curr = actual;
                            }
                        }

                        // Since we aren't using the weak variant of the atomic
                        // operation, the only possible option for `curr` is to
                        // also include the `NOTIFYING` bit.
                        debug_assert_eq!(curr, curr | NOTIFYING);

                        // Take the task to notify once the atomic operation has
                        // completed.
                        notify = (*self.task.get()).take();
                    }

                    if let Some(task) = notify {
                        task.notify();
                    }
                }
            }
            NOTIFYING => {
                // Currently in the process of notifying the task, i.e.,
                // `notify` is currently being called on the old task handle.
                // So, we call notify on the new task handle
                task.notify();
            }
            _ => {
                // TODO: Assert valid state
            }
        }
    }

    /// Notifies the task that last called `register`.
    ///
    /// If `register` has not been called yet, then this does nothing.
    pub fn notify(&self) {
        // AcqRel ordering is used in order to acquire the value of the `task`
        // cell as well as to establish a `release` ordering with whatever
        // memory the `AtomicTask` is associated with.
        match self.state.fetch_or(NOTIFYING, AcqRel) {
            WAITING => {
                // The notifying lock has been acquired.
                let task = unsafe { (*self.task.get()).take() };

                // Release the lock
                self.state.fetch_and(!NOTIFYING, Release);

                if let Some(task) = task {
                    task.notify();
                }
            }
            _ => {
                // There is a concurrent thread currently updating the
                // associated task.
                //
                // Nothing more to do as the `NOTIFYING` bit has been set
                //
                // TODO: Validate the state
            }
        }
    }
}

impl Default for AtomicTask {
    fn default() -> Self {
        AtomicTask::new()
    }
}

impl fmt::Debug for AtomicTask {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "AtomicTask")
    }
}

unsafe impl Send for AtomicTask {}
unsafe impl Sync for AtomicTask {}
