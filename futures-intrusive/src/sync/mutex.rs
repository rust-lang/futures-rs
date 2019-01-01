//! An asynchronously awaitable mutex for synchronization between concurrently
//! executing futures.

use futures_core::future::{Future, FusedFuture};
use futures_core::task::{LocalWaker, Poll, Waker};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::null_mut;
use core::cell::{UnsafeCell, RefCell};
use crate::intrusive_list::{LinkedList, ListNode};

/// Tracks how the future had interacted with the mutex
#[derive(PartialEq)]
enum PollState {
    /// The task has never interacted with the mutex.
    New,
    /// The task was added to the wait queue at the mutex.
    Waiting,
    /// The task had previously waited on the mutex, but was notified
    /// that the mutex was released in the meantime.
    Notified,
    /// The task had been polled to completion.
    Done,
}

/// Tracks the MutexLockFuture waiting state.
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

/// Internal state of the `Mutex`
struct MutexState {
    is_fair: bool,
    is_locked: bool,
    waiters: LinkedList<WaitQueueEntry>,
}

impl MutexState {
    fn new(is_fair: bool) -> Self {
        MutexState {
            is_fair,
            is_locked: false,
            waiters: LinkedList::new(),
        }
    }

    /// Wakes up the last waiter and removes it from the wait queue
    fn wakeup_last_waiter(&mut self) {
        let last_waiter =
            if self.is_fair {
                self.waiters.peek_last()
            } else {
                self.waiters.remove_last()
            };

        if last_waiter != null_mut() {
            // Notify the waiter that it can try to lock the mutex again.
            // The notification gets tracked inside the waiter.
            // If the waiter aborts it's wait (drops the future), another task
            // must be woken.
            unsafe {
                (*last_waiter).state = PollState::Notified;

                let task = (*last_waiter).task.take();
                if let Some(ref handle) = task {
                    handle.wake();
                }
            }
        }
    }

    fn is_locked(&self) -> bool {
        self.is_locked
    }

    /// Unlocks the mutex. This is expected to be only called from the current holder of the mutex
    fn unlock(&mut self) {
        if self.is_locked {
            self.is_locked = false;
            // TODO: Does this require a memory barrier for the actual data,
            // or is this covered by unlocking the mutex which protects the data?
            // Wakeup the last waiter
            self.wakeup_last_waiter();
        }
    }

    /// Tries to acquire the Mutex from a WaitQueueEntry.
    /// If it isn't available, the WaitQueueEntry gets added to the wait
    /// queue at the Mutex, and will be signalled once ready.
    /// This function is only safe as long as the `wait_node`s address is guaranteed
    /// to be stable until it gets removed from the queue.
    unsafe fn try_lock(
        &mut self,
        wait_node: &mut ListNode<WaitQueueEntry>,
        lw: &LocalWaker,
    ) -> Poll<()> {
        match wait_node.state {
            PollState::New => {
                // The fast path - the Mutex isn't locked by anyone else.
                // If the mutex is fair, noone must be in the wait list before us.
                if !self.is_locked && (!self.is_fair || self.waiters.is_empty()) {
                    self.is_locked = true;
                    wait_node.state = PollState::Done;
                    Poll::Ready(())
                }
                else {
                    // Add the task to the wait queue
                    wait_node.task = Some(lw.clone().into_waker());
                    wait_node.state = PollState::Waiting;
                    self.waiters.add_front(wait_node);
                    Poll::Pending
                }
            },
            PollState::Waiting => {
                // The MutexLockFuture is already in the queue.
                if self.is_fair {
                    // The task needs to wait until it gets notified in order to
                    // maintain the ordering.
                    Poll::Pending
                }
                else {
                    // For throughput improvement purposes, grab the lock immediately
                    // if it's available.
                    if !self.is_locked {
                        self.is_locked = true;
                        wait_node.state = PollState::Done;
                        // Since this waiter has been registered before, it must
                        // get removed from the waiter list.
                        self.force_remove_waiter(wait_node);
                        Poll::Ready(())
                    }
                    else {
                        Poll::Pending
                    }
                }
            },
            PollState::Notified => {
                // We had been woken by the mutex, since the mutex is available again.
                // The mutex thereby removed us from the waiters list.
                // Just try to lock again. If the mutex isn't available,
                // we need to add it to the wait queue again.
                if !self.is_locked {
                    if self.is_fair {
                        // In a fair Mutex, the WaitQueueEntry is kept in the
                        // linked list and must be removed here
                        self.force_remove_waiter(wait_node);
                    }
                    self.is_locked = true;
                    wait_node.state = PollState::Done;
                    Poll::Ready(())
                }
                else {
                    // Add to queue
                    wait_node.task = Some(lw.clone().into_waker());
                    wait_node.state = PollState::Waiting;
                    self.waiters.add_front(wait_node);
                    Poll::Pending
                }

            },
            PollState::Done => {
                // The future had been polled to completion before
                panic!("polled Mutex after completion");
            },
        }
    }

    /// Tries to remove a waiter from the wait queue, and panics if the
    /// waiter is no longer valid.
    unsafe fn force_remove_waiter(&mut self, wait_node: *mut ListNode<WaitQueueEntry>) {
        if !self.waiters.remove(wait_node) {
            // Panic if the address isn't found. This can only happen if the contract was
            // violated, e.g. the WaitQueueEntry got moved after the initial poll.
            panic!("Future could not be removed from wait queue");
        }
    }

    /// Removes the waiter from the list.
    /// This function is only safe as long as the reference that is passed here
    /// equals the reference/address under which the waiter was added.
    /// The waiter must not have been moved in between.
    fn remove_waiter(&mut self, wait_node: &mut ListNode<WaitQueueEntry>) {
        // MutexLockFuture only needs to get removed if it had been added to
        // the wait queue of the Mutex. This has happened in the PollState::Waiting case.
        // If the current waiter was notitfied, another waiter must get notified now.
        match wait_node.state {
            PollState::Notified => {
                if self.is_fair {
                    // In a fair Mutex, the WaitQueueEntry is kept in the
                    // linked list and must be removed here
                    unsafe { self.force_remove_waiter(wait_node) };
                }
                wait_node.state = PollState::Done;
                self.wakeup_last_waiter();
            },
            PollState::Waiting => {
                // Remove the WaitQueueEntry from the linked list
                unsafe { self.force_remove_waiter(wait_node) };
                wait_node.state = PollState::Done;
            },
            PollState::New | PollState::Done => {},
        }
    }
}

/// An RAII guard returned by the `lock` and `try_lock` methods.
/// When this structure is dropped (falls out of scope), the lock will be
/// unlocked.
pub struct LocalMutexGuard<'a, T: 'a> {
    /// The Mutex which is associated with this Guard
    mutex: &'a LocalMutex<T>,
}

impl<T: core::fmt::Debug> core::fmt::Debug for LocalMutexGuard<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalMutexGuard")
            .finish()
    }
}

impl<T> Drop for LocalMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Release the mutex
        self.mutex.state.borrow_mut().unlock();
    }
}

impl<T> Deref for LocalMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for LocalMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.value.get() }
    }
}

/// A future which resolves when the target mutex has been successfully acquired.
#[must_use = "futures do nothing unless polled"]
pub struct LocalMutexLockFuture<'a, T: 'a> {
    /// The Mutex which should get locked trough this Future
    mutex: Option<&'a LocalMutex<T>>,
    /// Node for waiting at the mutex
    wait_node: ListNode<WaitQueueEntry>,
}

impl<'a, T: core::fmt::Debug> core::fmt::Debug for LocalMutexLockFuture<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalMutexLockFuture")
            .finish()
    }
}

impl<'a, T> Future for LocalMutexLockFuture<'a, T> {
    type Output = LocalMutexGuard<'a, T>;

    fn poll(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Self::Output> {
        // Safety: The next operations are safe, because Pin promises us that
        // the address of the wait queue entry inside MutexLocalFuture is stable,
        // and we don't move any fields inside the future until it gets dropped.
        let mut_self: &mut LocalMutexLockFuture<T> = unsafe {
            Pin::get_unchecked_mut(self)
        };

        let mutex = mut_self.mutex.expect("polled LocalMutexLockFuture after completion");
        let mut mutex_state = mutex.state.borrow_mut();

        let poll_res = unsafe {
            mutex_state.try_lock(
            &mut mut_self.wait_node,
            lw)
        };

        match poll_res {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                // The mutex was acquired
                mut_self.mutex = None;
                Poll::Ready(LocalMutexGuard::<'a, T>{
                    mutex,
                })
            },
        }
    }
}

impl<'a, T> FusedFuture for LocalMutexLockFuture<'a, T> {
   fn is_terminated(&self) -> bool {
       self.mutex.is_none()
   }
}

impl<'a, T> Drop for LocalMutexLockFuture<'a, T> {
    fn drop(&mut self) {
        // If this LocalMutexLockFuture has been polled and it was added to the
        // wait queue at the mutex, it must be removed before dropping.
        // Otherwise the mutex would access invalid memory.
        if let Some(mutex) = self.mutex {
            let mut mutex_state = mutex.state.borrow_mut();
            mutex_state.remove_waiter(&mut self.wait_node);
        }
    }
}

/// A futures-aware mutex.
pub struct LocalMutex<T> {
    value: UnsafeCell<T>,
    state: RefCell<MutexState>,
}

// It is safe to send mutexes between threads, as long as they are not used and
// thereby borrowed
unsafe impl<T: Send> Send for LocalMutex<T> {}

impl<T: core::fmt::Debug> core::fmt::Debug for LocalMutex<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalMutex")
            .field("is_locked", &format_args!("{}", self.is_locked()))
            .finish()
    }
}

impl<T> LocalMutex<T> {
    /// Creates a new futures-aware mutex.
    ///
    /// `is_fair` defines whether the `Mutex` should behave be fair regarding the
    /// order of waiters. A fair `Mutex` will only allow the first waiter which
    /// tried to lock but failed to lock the `Mutex` once it's available again.
    /// Other waiters must wait until either this locking attempt completes, and
    /// the `Mutex` gets unlocked again, or until the `MutexLockFuture` which
    /// tried to gain the lock is dropped.
    pub fn new(value: T, is_fair: bool) -> LocalMutex<T> {
        LocalMutex::<T> {
            value: UnsafeCell::new(value),
            state: RefCell::new(MutexState::new(is_fair)),
        }
    }

    /// Acquire the mutex asynchronously.
    ///
    /// This method returns a future that will resolve once the mutex has been
    /// successfully acquired.
    pub fn lock(&self) -> LocalMutexLockFuture<'_, T> {
        LocalMutexLockFuture {
            mutex: Some(&self),
            wait_node: ListNode::new(WaitQueueEntry::new()),
        }
    }

    /// Returns whether the mutex is locked.
    pub fn is_locked(&self) -> bool {
        self.state.borrow().is_locked()
    }
}

#[cfg(feature = "std")]
mod if_std {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// An RAII guard returned by the `lock` and `try_lock` methods.
    /// When this structure is dropped (falls out of scope), the lock will be
    /// unlocked.
    pub struct MutexGuard<'a, T: 'a> {
        /// The Mutex which is associated with this Guard
        mutex: &'a Mutex<T>,
    }

    impl<T: core::fmt::Debug> core::fmt::Debug for MutexGuard<'_, T> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("MutexGuard")
                .finish()
        }
    }

    impl<T> Drop for MutexGuard<'_, T> {
        fn drop(&mut self) {
            // Release the mutex
            self.mutex.state.lock().unwrap().unlock();
        }
    }

    impl<T> Deref for MutexGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T {
            unsafe { &*self.mutex.value.get() }
        }
    }

    impl<T> DerefMut for MutexGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut T {
            unsafe { &mut *self.mutex.value.get() }
        }
    }

    /// A future which resolves when the target mutex has been successfully acquired.
    #[must_use = "futures do nothing unless polled"]
    pub struct MutexLockFuture<'a, T: 'a> {
        /// The Mutex which should get locked trough this Future
        mutex: Option<&'a Mutex<T>>,
        /// Node for waiting at the mutex
        wait_node: ListNode<WaitQueueEntry>,
    }

    impl<'a, T: core::fmt::Debug> core::fmt::Debug for MutexLockFuture<'a, T> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("MutexLockFuture")
                .finish()
        }
    }

    impl<'a, T> Future for MutexLockFuture<'a, T> {
        type Output = MutexGuard<'a, T>;

        fn poll(
            self: Pin<&mut Self>,
            lw: &LocalWaker,
        ) -> Poll<Self::Output> {
            // Safety: The next operations are safe, because Pin promises us that
            // the address of the wait queue entry inside MutexLocalFuture is stable,
            // and we don't move any fields inside the future until it gets dropped.
            let mut_self: &mut MutexLockFuture<T> = unsafe {
                Pin::get_unchecked_mut(self)
            };

            let mutex = mut_self.mutex.expect("polled MutexLockFuture after completion");
            let mut mutex_state = mutex.state.lock().unwrap();

            let poll_res = unsafe {
                mutex_state.try_lock(
                &mut mut_self.wait_node,
                lw)
            };

            match poll_res {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    // The mutex was acquired
                    mut_self.mutex = None;
                    Poll::Ready(MutexGuard::<'a, T>{
                        mutex,
                    })
                },
            }
        }
    }

    impl<'a, T> FusedFuture for MutexLockFuture<'a, T> {
        fn is_terminated(&self) -> bool {
            self.mutex.is_none()
        }
    }

    impl<'a, T> Drop for MutexLockFuture<'a, T> {
        fn drop(&mut self) {
            // If this MutexLockFuture has been polled and it was added to the
            // wait queue at the mutex, it must be removed before dropping.
            // Otherwise the mutex would access invalid memory.
            if let Some(mutex) = self.mutex {
                let mut mutex_state = mutex.state.lock().unwrap();
                mutex_state.remove_waiter(&mut self.wait_node);
            }
        }
    }

    /// A futures-aware mutex.
    pub struct Mutex<T> {
        value: UnsafeCell<T>,
        state: StdMutex<MutexState>,
    }

    // Mutexes can be moved freely between threads and acquired on any thread so long
    // as the inner value can be safely sent between threads.
    // Automatic derive doesn't work due to the unsafe pointer in WaitQueueEntry
    unsafe impl<T: Send> Send for Mutex<T> {}
    unsafe impl<T: Send> Sync for Mutex<T> {}

    impl<T: core::fmt::Debug> core::fmt::Debug for Mutex<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("Mutex")
                .field("is_locked", &format_args!("{}", self.is_locked()))
                .finish()
        }
    }

    impl<T> Mutex<T> {
        /// Creates a new futures-aware mutex.
        ///
        /// `is_fair` defines whether the `Mutex` should behave be fair regarding the
        /// order of waiters. A fair `Mutex` will only allow the first waiter which
        /// tried to lock but failed to lock the `Mutex` once it's available again.
        /// Other waiters must wait until either this locking attempt completes, and
        /// the `Mutex` gets unlocked again, or until the `MutexLockFuture` which
        /// tried to gain the lock is dropped.
        pub fn new(value: T, is_fair: bool) -> Mutex<T> {
            Mutex::<T> {
                value: UnsafeCell::new(value),
                state: StdMutex::new(MutexState::new(is_fair)),
            }
        }

        /// Acquire the mutex asynchronously.
        ///
        /// This method returns a future that will resolve once the mutex has been
        /// successfully acquired.
        pub fn lock(&self) -> MutexLockFuture<'_, T> {
            MutexLockFuture {
                mutex: Some(&self),
                wait_node: ListNode::new(WaitQueueEntry::new()),
            }
        }

        /// Returns whether the mutex is locked.
        pub fn is_locked(&self) -> bool {
            self.state.lock().unwrap().is_locked()
        }
    }

}

#[cfg(feature = "std")]
pub use self::if_std::*;