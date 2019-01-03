//! An asynchronously awaitable oneshot channel

use futures_core::future::{Future, FusedFuture};
use futures_core::task::{LocalWaker, Poll, Waker};
use core::pin::Pin;
use core::cell::RefCell;
use crate::intrusive_list::{LinkedList, ListNode};

/// Tracks how the future had interacted with the channel
#[derive(PartialEq)]
enum PollState {
    /// The task is not registered at the wait queue at the channel
    Unregistered,
    /// The task was added to the wait queue at the channel.
    Registered,
}

/// Tracks the oneshot channel futures waiting state.
/// Access to this struct is synchronized through the mutex in the channel.
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
            state: PollState::Unregistered,
        }
    }
}

/// Internal state of the oneshot channel
struct ChannelState<T> {
    /// Whether the channel had been fulfilled before
    is_fulfilled: bool,
    /// The value which is stored inside the channel
    value: Option<T>,
    /// The list of waiters, which are waiting for the channel to get fulfilled
    waiters: LinkedList<WaitQueueEntry>,
}

impl<T> ChannelState<T> {
    fn new() -> ChannelState<T> {
        ChannelState::<T> {
            is_fulfilled: false,
            value: None,
            waiters: LinkedList::new(),
        }
    }

    /// Writes a single value to the channel.
    /// If a value had been written to the channel before, the new value will be rejected.
    fn send(&mut self, value: T) -> Result<(), T> {
        if self.is_fulfilled {
            return Err(value);
        }

        self.value = Some(value);
        self.is_fulfilled = true;

        // Wakeup all waiters
        let mut waiters = self.waiters.take();

        unsafe {
            // Reverse the waiter list, so that the oldest waker (which is
            // at the end of the list), gets woken first and has the best
            // chance to grab the channel value.
            waiters.reverse();

            for waiter in waiters.into_iter() {
                let task = (*waiter).task.take();
                if let Some(ref handle) = task {
                    handle.wake();
                }
                (*waiter).state = PollState::Unregistered;
            }
        }

        Ok(())
    }

    /// Tries to read the value from the channel.
    /// If the value isn't available yet, the LocalOneshotReceiveFuture gets added to the
    /// wait queue at the channel, and will be signalled once ready.
    /// This function is only safe as long as the `wait_node`s address is guaranteed
    /// to be stable until it gets removed from the queue.
    unsafe fn try_receive(
        &mut self,
        wait_node: &mut ListNode<WaitQueueEntry>,
        lw: &LocalWaker,
    ) -> Poll<Option<T>> {
        match wait_node.state {
            PollState::Unregistered => {
                let maybe_val = self.value.take();
                match maybe_val {
                    Some(v) => {
                        // A value was available inside the channel and was fetched
                        Poll::Ready(Some(v))
                    },
                    None => {
                        // Check if something was written into the channel before
                        if self.is_fulfilled {
                            Poll::Ready(None)
                        }
                        else {
                            // Added the task to the wait queue
                            wait_node.task = Some(lw.clone().into_waker());
                            wait_node.state = PollState::Registered;
                            self.waiters.add_front(wait_node);
                            Poll::Pending
                        }
                    },
                }
            },
            PollState::Registered => {
                // Since the channel wakes up all waiters and moves their states to unregistered
                // there can't be any value in the channel in this state.
                Poll::Pending
            },
        }
    }

    fn remove_waiter(&mut self, wait_node: &mut ListNode<WaitQueueEntry>) {
        // LocalOneshotReceiveFuture only needs to get removed if it had been added to
        // the wait queue of the channel. This has happened in the PollState::Waiting case.
        if let PollState::Registered = wait_node.state {
            if ! unsafe { self.waiters.remove(wait_node) } {
                // Panic if the address isn't found. This can only happen if the contract was
                // violated, e.g. the WaitQueueEntry got moved after the initial poll.
                panic!("Future could not be removed from wait queue");
            }
            wait_node.state = PollState::Unregistered;
        }
    }
}

/// A channel which can be used to exchange a single value between two
/// concurrent tasks.
///
/// Tasks can wait for the value to get delivered via `receive`.
/// The returned Future will get fulfilled when a value is sent into the channel.
pub struct LocalOneshotChannel<T> {
    inner: RefCell<ChannelState<T>>,
}

// The channel can be sent to other threads as long as it's not borrowed and the
// value in it can be sent to other threads.
unsafe impl<T: Send> Send for LocalOneshotChannel<T> {}

impl<T> core::fmt::Debug for LocalOneshotChannel<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalOneshotChannel")
            .finish()
    }
}

impl<T> LocalOneshotChannel<T> {
    /// Creates a new LocalOneshotChannel in the given state
    pub fn new() -> LocalOneshotChannel<T> {
        LocalOneshotChannel {
            inner: RefCell::new(ChannelState::new()),
        }
    }

    /// Writes a single value to the channel.
    ///
    /// This will notify waiters about the availability of the value.
    /// If a value had been written to the channel before, the new value will be rejected
    /// and returned inside the error variant.
    pub fn send(&self, value: T) -> Result<(), T> {
        self.inner.borrow_mut().send(value)
    }

    /// Returns a future that gets fulfilled when a value is written to the channel.
    pub fn receive(&self) -> LocalOneshotReceiveFuture<T> {
        LocalOneshotReceiveFuture {
            channel: Some(&self),
            wait_node: ListNode::new(WaitQueueEntry::new()),
        }
    }
}

/// A Future that is resolved once the corresponding LocalOneshotChannel has been set
#[must_use = "futures do nothing unless polled"]
pub struct LocalOneshotReceiveFuture<'a, T> {
    /// The LocalOneshotChannel that is associated with this LocalOneshotReceiveFuture
    channel: Option<&'a LocalOneshotChannel<T>>,
    /// Node for waiting on the channel
    wait_node: ListNode<WaitQueueEntry>,
}

impl<'a, T> core::fmt::Debug for LocalOneshotReceiveFuture<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("LocalOneshotReceiveFuture")
            .finish()
    }
}

impl<'a, T> Future for LocalOneshotReceiveFuture<'a, T> {
    type Output = Option<T>;

    fn poll(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<T>> {
        // It might be possible to use Pin::map_unchecked here instead of the two unsafe APIs.
        // However this didn't seem to work for some borrow checker reasons

        // Safety: The next operations are safe, because Pin promises us that
        // the address of the wait queue entry inside LocalOneshotReceiveFuture is stable,
        // and we don't move any fields inside the future until it gets dropped.
        let mut_self: &mut LocalOneshotReceiveFuture<T> = unsafe {
            Pin::get_unchecked_mut(self)
        };

        let channel = mut_self.channel.expect("polled LocalOneshotReceiveFuture after completion");

        let poll_res = unsafe {
            channel.inner.borrow_mut().try_receive(
                &mut mut_self.wait_node,
                lw)
        };

        if poll_res.is_ready() {
            // A value was available
            mut_self.channel = None;
        }

        poll_res
    }
}

impl<'a, T> FusedFuture for LocalOneshotReceiveFuture<'a, T> {
    fn is_terminated(&self) -> bool {
        self.channel.is_none()
    }
}

impl<'a, T> Drop for LocalOneshotReceiveFuture<'a, T> {
    fn drop(&mut self) {
        // If this LocalOneshotReceiveFuture has been polled and it was added to the
        // wait queue at the channel, it must be removed before dropping.
        // Otherwise the channel would access invalid memory.
        if let Some(channel) = self.channel {
            channel.inner.borrow_mut().remove_waiter(&mut self.wait_node);
        }
    }
}

#[cfg(feature = "std")]
mod if_std {
    use super::*;
    use std::sync::Mutex;

    /// A channel which can be used to exchange a single value between two
    /// concurrent tasks.
    ///
    /// Tasks can wait for the value to get delivered via `receive`.
    /// The returned Future will get fulfilled when a value is sent into the channel.
    pub struct OneshotChannel<T> {
        inner: Mutex<ChannelState<T>>,
    }

    // The channel can be sent to other threads as long as it's not borrowed and the
    // value in it can be sent to other threads.
    unsafe impl<T: Send> Send for OneshotChannel<T> {}

    impl<T> core::fmt::Debug for OneshotChannel<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("OneshotChannel")
                .finish()
        }
    }

    impl<T> OneshotChannel<T> {
        /// Creates a new OneshotChannel in the given state
        pub fn new() -> OneshotChannel<T> {
            OneshotChannel {
                inner: Mutex::new(ChannelState::new()),
            }
        }

        /// Writes a single value to the channel.
        ///
        /// This will notify waiters about the availability of the value.
        /// If a value had been written to the channel before, the new value will be rejected
        /// and returned inside the error variant.
        pub fn send(&self, value: T) -> Result<(), T> {
            self.inner.lock().unwrap().send(value)
        }

        /// Returns a future that gets fulfilled when a value is written to the channel.
        pub fn receive(&self) -> OneshotReceiveFuture<T> {
            OneshotReceiveFuture {
                channel: Some(&self),
                wait_node: ListNode::new(WaitQueueEntry::new()),
            }
        }
    }

    /// A Future that is resolved once the corresponding OneshotChannel has been set
    #[must_use = "futures do nothing unless polled"]
    pub struct OneshotReceiveFuture<'a, T> {
        /// The OneshotChannel that is associated with this OneshotReceiveFuture
        channel: Option<&'a OneshotChannel<T>>,
        /// Node for waiting on the channel
        wait_node: ListNode<WaitQueueEntry>,
    }

    impl<'a, T> core::fmt::Debug for OneshotReceiveFuture<'a, T> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("OneshotReceiveFuture")
                .finish()
        }
    }

    impl<'a, T> Future for OneshotReceiveFuture<'a, T> {
        type Output = Option<T>;

        fn poll(
            self: Pin<&mut Self>,
            lw: &LocalWaker,
        ) -> Poll<Option<T>> {
            // It might be possible to use Pin::map_unchecked here instead of the two unsafe APIs.
            // However this didn't seem to work for some borrow checker reasons

            // Safety: The next operations are safe, because Pin promises us that
            // the address of the wait queue entry inside OneshotReceiveFuture is stable,
            // and we don't move any fields inside the future until it gets dropped.
            let mut_self: &mut OneshotReceiveFuture<T> = unsafe {
                Pin::get_unchecked_mut(self)
            };

            let channel = mut_self.channel.expect("polled OneshotReceiveFuture after completion");

            let poll_res = unsafe {
                channel.inner.lock().unwrap().try_receive(
                    &mut mut_self.wait_node,
                    lw)
            };

            if poll_res.is_ready() {
                // A value was available
                mut_self.channel = None;
            }

            poll_res
        }
    }

    impl<'a, T> FusedFuture for OneshotReceiveFuture<'a, T> {
        fn is_terminated(&self) -> bool {
            self.channel.is_none()
        }
    }

    impl<'a, T> Drop for OneshotReceiveFuture<'a, T> {
        fn drop(&mut self) {
            // If this OneshotReceiveFuture has been polled and it was added to the
            // wait queue at the channel, it must be removed before dropping.
            // Otherwise the channel would access invalid memory.
            if let Some(channel) = self.channel {
                channel.inner.lock().unwrap().remove_waiter(&mut self.wait_node);
            }
        }
    }
}

#[cfg(feature = "std")]
pub use self::if_std::*;