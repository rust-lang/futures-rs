use super::{SendError, TrySendError, SendErrorKind, SenderTask, Inner, decode_state, encode_state, MAX_CAPACITY};
use futures_core::task::{self, Poll};
use std::marker::Unpin;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering::SeqCst;

/// The transmission end of a bounded mpsc channel.
///
/// This value is created by the [`channel`](channel) function.
#[derive(Debug)]
pub struct Sender<T> {
    // Channel state shared between the sender and receiver.
    pub(super) inner: Arc<Inner<T>>,

    // Handle to the task that is blocked on this sender. This handle is sent
    // to the receiver half in order to be notified when the sender becomes
    // unblocked.
    pub(super) sender_task: Arc<Mutex<SenderTask>>,

    // True if the sender might be blocked. This is an optimization to avoid
    // having to lock the mutex most of the time.
    pub(super) maybe_parked: bool,
}

// We never project PinMut<Sender> to `PinMut<T>`
impl<T> Unpin for Sender<T> {}

/// The transmission end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`](unbounded) function.
#[derive(Debug)]
pub struct UnboundedSender<T>(pub(super) Sender<T>);

trait AssertKinds: Send + Sync + Clone {}
impl AssertKinds for UnboundedSender<u32> {}

impl<T> Sender<T> {
    /// Attempts to send a message on this `Sender`, returning the message
    /// if there was an error.
    pub fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        // If the sender is currently blocked, reject the message
        if !self.poll_unparked(None).is_ready() {
            return Err(TrySendError {
                err: SendError {
                    kind: SendErrorKind::Full,
                },
                val: msg,
            });
        }

        // The channel has capacity to accept the message, so send it
        self.do_send(None, msg)
    }

    /// Send a message on the channel.
    ///
    /// This function should only be called after
    /// [`poll_ready`](Sender::poll_ready) has reported that the channel is
    /// ready to receive a message.
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        self.try_send(msg)
            .map_err(|e| e.err)
    }

    // Do the send without failing
    // None means close
    fn do_send(&mut self, cx: Option<&mut task::Context>, msg: T)
        -> Result<(), TrySendError<T>>
    {
        // Anyone callig do_send *should* make sure there is room first,
        // but assert here for tests as a sanity check.
        debug_assert!(self.poll_unparked(None).is_ready());

        // First, increment the number of messages contained by the channel.
        // This operation will also atomically determine if the sender task
        // should be parked.
        //
        // None is returned in the case that the channel has been closed by the
        // receiver. This happens when `Receiver::close` is called or the
        // receiver is dropped.
        let park_self = match self.inc_num_messages(false) {
            Some(park_self) => park_self,
            None => return Err(TrySendError {
                err: SendError {
                    kind: SendErrorKind::Disconnected,
                },
                val: msg,
            }),
        };

        // If the channel has reached capacity, then the sender task needs to
        // be parked. This will send the task handle on the parked task queue.
        //
        // However, when `do_send` is called while dropping the `Sender`,
        // `task::current()` can't be called safely. In this case, in order to
        // maintain internal consistency, a blank message is pushed onto the
        // parked task queue.
        if park_self {
            self.park(cx);
        }

        self.queue_push_and_signal(Some(msg));

        Ok(())
    }

    // Do the send without parking current task.
    fn do_send_nb(&self, msg: Option<T>) -> Result<(), TrySendError<T>> {
        match self.inc_num_messages(msg.is_none()) {
            Some(park_self) => assert!(!park_self),
            None => {
                // The receiver has closed the channel. Only abort if actually
                // sending a message. It is important that the stream
                // termination (None) is always sent. This technically means
                // that it is possible for the queue to contain the following
                // number of messages:
                //
                //     num-senders + buffer + 1
                //
                if let Some(msg) = msg {
                    return Err(TrySendError {
                        err: SendError {
                            kind: SendErrorKind::Disconnected,
                        },
                        val: msg,
                    });
                } else {
                    return Ok(());
                }
            },
        };

        self.queue_push_and_signal(msg);

        Ok(())
    }

    fn poll_ready_nb(&self) -> Poll<Result<(), SendError>> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if state.is_open {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(SendError {
                kind: SendErrorKind::Full,
            }))
        }
    }


    // Push message to the queue and signal to the receiver
    fn queue_push_and_signal(&self, msg: Option<T>) {
        // Push the message onto the message queue
        self.inner.message_queue.push(msg);

        // Signal to the receiver that a message has been enqueued. If the
        // receiver is parked, this will unpark the task.
        self.signal();
    }

    // Increment the number of queued messages. Returns if the sender should
    // block.
    fn inc_num_messages(&self, close: bool) -> Option<bool> {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            // The receiver end closed the channel.
            if !state.is_open {
                return None;
            }

            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(state.num_messages < MAX_CAPACITY, "buffer space \
                    exhausted; sending this messages would overflow the state");

            state.num_messages += 1;

            // The channel is closed by all sender handles being dropped.
            if close {
                state.is_open = false;
            }

            let next = encode_state(&state);
            match self.inner.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => {
                    // Block if the current number of pending messages has exceeded
                    // the configured buffer size
                    let park_self = !close && match self.inner.buffer {
                        Some(buffer) => state.num_messages > buffer,
                        None => false,
                    };

                    return Some(park_self)
                }
                Err(actual) => curr = actual,
            }
        }
    }

    // Signal to the receiver task that a message has been enqueued
    fn signal(&self) {
        // TODO
        // This logic can probably be improved by guarding the lock with an
        // atomic.
        //
        // Do this step first so that the lock is dropped when
        // `unpark` is called
        let task = {
            let mut recv_task = self.inner.recv_task.lock().unwrap();

            // If the receiver has already been unparked, then there is nothing
            // more to do
            if recv_task.unparked {
                return;
            }

            // Setting this flag enables the receiving end to detect that
            // an unpark event happened in order to avoid unnecessarily
            // parking.
            recv_task.unparked = true;
            recv_task.task.take()
        };

        if let Some(task) = task {
            task.wake();
        }
    }

    fn park(&mut self, cx: Option<&mut task::Context>) {
        // TODO: clean up internal state if the task::current will fail

        let task = cx.map(|cx| cx.waker().clone());

        {
            let mut sender = self.sender_task.lock().unwrap();
            sender.task = task;
            sender.is_parked = true;
        }

        // Send handle over queue
        let t = self.sender_task.clone();
        self.inner.parked_queue.push(t);

        // Check to make sure we weren't closed after we sent our task on the
        // queue
        let state = decode_state(self.inner.state.load(SeqCst));
        self.maybe_parked = state.is_open;
    }

    /// Polls the channel to determine if there is guaranteed capacity to send
    /// at least one item without waiting.
    ///
    /// # Return value
    ///
    /// This method returns:
    ///
    /// - `Ok(Async::Ready(_))` if there is sufficient capacity;
    /// - `Ok(Async::Pending)` if the channel may not have
    ///   capacity, in which case the current task is queued to be notified once
    ///   capacity is available;
    /// - `Err(SendError)` if the receiver has been dropped.
    pub fn poll_ready(
        &mut self,
        cx: &mut task::Context
    ) -> Poll<Result<(), SendError>> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if !state.is_open {
            return Poll::Ready(Err(SendError {
                kind: SendErrorKind::Disconnected,
            }));
        }

        self.poll_unparked(Some(cx)).map(Ok)
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        !decode_state(self.inner.state.load(SeqCst)).is_open
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&mut self) {
        // There's no need to park this sender, its dropping,
        // and we don't want to check for capacity, so skip
        // that stuff from `do_send`.

        let _ = self.do_send_nb(None);
    }

    fn poll_unparked(&mut self, cx: Option<&mut task::Context>) -> Poll<()> {
        // First check the `maybe_parked` variable. This avoids acquiring the
        // lock in most cases
        if self.maybe_parked {
            // Get a lock on the task handle
            let mut task = self.sender_task.lock().unwrap();

            if !task.is_parked {
                self.maybe_parked = false;
                return Poll::Ready(())
            }

            // At this point, an unpark request is pending, so there will be an
            // unpark sometime in the future. We just need to make sure that
            // the correct task will be notified.
            //
            // Update the task in case the `Sender` has been moved to another
            // task
            task.task = cx.map(|cx| cx.waker().clone());

            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

impl<T> UnboundedSender<T> {
    /// Check if the channel is ready to receive a message.
    pub fn poll_ready(
        &self,
        _: &mut task::Context,
    ) -> Poll<Result<(), SendError>> {
        self.0.poll_ready_nb()
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&self) {
        // There's no need to park this sender, its dropping,
        // and we don't want to check for capacity, so skip
        // that stuff from `do_send`.

        let _ = self.0.do_send_nb(None);
    }

    /// Send a message on the channel.
    ///
    /// This method should only be called after `poll_ready` has been used to
    /// verify that the channel is ready to receive a message.
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        self.0.do_send_nb(Some(msg))
            .map_err(|e| e.err)
    }

    /// Sends a message along this channel.
    ///
    /// This is an unbounded sender, so this function differs from `Sink::send`
    /// by ensuring the return type reflects that the channel is always ready to
    /// receive messages.
    pub fn unbounded_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        self.0.do_send_nb(Some(msg))
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> UnboundedSender<T> {
        UnboundedSender(self.0.clone())
    }
}


impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        // Since this atomic op isn't actually guarding any memory and we don't
        // care about any orderings besides the ordering on the single atomic
        // variable, a relaxed ordering is acceptable.
        let mut curr = self.inner.num_senders.load(SeqCst);

        loop {
            // If the maximum number of senders has been reached, then fail
            if curr == self.inner.max_senders() {
                panic!("cannot clone `Sender` -- too many outstanding senders");
            }

            debug_assert!(curr < self.inner.max_senders());

            let next = curr + 1;
            let actual = self.inner.num_senders.compare_and_swap(curr, next, SeqCst);

            // The ABA problem doesn't matter here. We only care that the
            // number of senders never exceeds the maximum.
            if actual == curr {
                return Sender {
                    inner: self.inner.clone(),
                    sender_task: Arc::new(Mutex::new(SenderTask::new())),
                    maybe_parked: false,
                };
            }

            curr = actual;
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // Ordering between variables don't matter here
        let prev = self.inner.num_senders.fetch_sub(1, SeqCst);

        if prev == 1 {
            // There's no need to park this sender, its dropping,
            // and we don't want to check for capacity, so skip
            // that stuff from `do_send`.
            let _ = self.do_send_nb(None);
        }
    }
}
