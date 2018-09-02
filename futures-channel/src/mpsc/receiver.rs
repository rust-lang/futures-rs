use super::{PopResult, Inner, decode_state, encode_state, TryRecvError};
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use std::marker::Unpin;
use std::pin::PinMut;
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::thread;

/// The receiving end of a bounded mpsc channel.
///
/// This value is created by the [`channel`](channel) function.
#[derive(Debug)]
pub struct Receiver<T> {
    pub(super) inner: Arc<Inner<T>>,
}

/// The receiving end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`](unbounded) function.
#[derive(Debug)]
pub struct UnboundedReceiver<T>(pub(super) Receiver<T>);

// `PinMut<UnboundedReceiver<T>>` is never projected to `PinMut<T>`
impl<T> Unpin for UnboundedReceiver<T> {}

// Returned from Receiver::try_park()
enum TryPark {
    Parked,
    Closed,
    NotEmpty,
}

impl<T> Receiver<T> {
    /// Closes the receiving half of a channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            if !state.is_open {
                break
            }

            state.is_open = false;

            let next = encode_state(&state);
            match self.inner.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(actual) => curr = actual,
            }
        }

        // Wake up any threads waiting as they'll see that we've closed the
        // channel and will continue on their merry way.
        loop {
            match unsafe { self.inner.parked_queue.pop() } {
                PopResult::Data(task) => {
                    task.lock().unwrap().notify();
                }
                PopResult::Empty => break,
                PopResult::Inconsistent => thread::yield_now(),
            }
        }
    }

    /// Tries to receive the next message without notifying a context if empty.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only when you've otherwise arranged to be notified when the channel is
    /// no longer empty.
    pub fn try_next(&mut self) -> Result<Option<T>, TryRecvError> {
        match self.next_message() {
            Poll::Ready(msg) => {
                Ok(msg)
            },
            Poll::Pending => Err(TryRecvError { _inner: () }),
        }
    }

    fn next_message(&mut self) -> Poll<Option<T>> {
        // Pop off a message
        loop {
            match unsafe { self.inner.message_queue.pop() } {
                PopResult::Data(msg) => {
                    // If there are any parked task handles in the parked queue,
                    // pop one and unpark it.
                    self.unpark_one();

                    // Decrement number of messages
                    self.dec_num_messages();

                    return Poll::Ready(msg);
                }
                PopResult::Empty => {
                    // The queue is empty, return Pending
                    return Poll::Pending;
                }
                PopResult::Inconsistent => {
                    // Inconsistent means that there will be a message to pop
                    // in a short time. This branch can only be reached if
                    // values are being produced from another thread, so there
                    // are a few ways that we can deal with this:
                    //
                    // 1) Spin
                    // 2) thread::yield_now()
                    // 3) task::current().unwrap() & return Pending
                    //
                    // For now, thread::yield_now() is used, but it would
                    // probably be better to spin a few times then yield.
                    thread::yield_now();
                }
            }
        }
    }

    // Unpark a single task handle if there is one pending in the parked queue
    fn unpark_one(&mut self) {
        loop {
            match unsafe { self.inner.parked_queue.pop() } {
                PopResult::Data(task) => {
                    task.lock().unwrap().notify();
                    return;
                }
                PopResult::Empty => {
                    // Queue empty, no task to wake up.
                    return;
                }
                PopResult::Inconsistent => {
                    // Same as above
                    thread::yield_now();
                }
            }
        }
    }

    // Try to park the receiver task
    fn try_park(&self, cx: &mut task::Context) -> TryPark {
        let curr = self.inner.state.load(SeqCst);
        let state = decode_state(curr);

        // If the channel is closed, then there is no need to park.
        if !state.is_open && state.num_messages == 0 {
            return TryPark::Closed;
        }

        // First, track the task in the `recv_task` slot
        let mut recv_task = self.inner.recv_task.lock().unwrap();

        if recv_task.unparked {
            // Consume the `unpark` signal without actually parking
            recv_task.unparked = false;
            return TryPark::NotEmpty;
        }

        recv_task.task = Some(cx.waker().clone());
        TryPark::Parked
    }

    fn dec_num_messages(&self) {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            state.num_messages -= 1;

            let next = encode_state(&state);
            match self.inner.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(actual) => curr = actual,
            }
        }
    }
}

// The receiver does not ever take a PinMut to the inner T
impl<T> Unpin for Receiver<T> {}

impl<T> Stream for Receiver<T> {
    type Item = T;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<T>> {
        loop {
            // Try to read a message off of the message queue.
            let msg = match self.next_message() {
                Poll::Ready(msg) => msg,
                Poll::Pending => {
                    // There are no messages to read, in this case, attempt to
                    // park. The act of parking will verify that the channel is
                    // still empty after the park operation has completed.
                    match self.try_park(cx) {
                        TryPark::Parked => {
                            // The task was parked, and the channel is still
                            // empty, return Pending.
                            return Poll::Pending;
                        }
                        TryPark::Closed => {
                            // The channel is closed, there will be no further
                            // messages.
                            return Poll::Ready(None);
                        }
                        TryPark::NotEmpty => {
                            // A message has been sent while attempting to
                            // park. Loop again, the next iteration is
                            // guaranteed to get the message.
                            continue;
                        }
                    }
                }
            };
            // Return the message
            return Poll::Ready(msg);
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        while self.next_message().is_ready() {
            // ...
        }
    }
}

impl<T> UnboundedReceiver<T> {
    /// Closes the receiving half of the channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        self.0.close();
    }

    /// Tries to receive the next message without notifying a context if empty.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only when you've otherwise arranged to be notified when the channel is
    /// no longer empty.
    pub fn try_next(&mut self) -> Result<Option<T>, TryRecvError> {
        self.0.try_next()
    }
}

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<T>> {
        PinMut::new(&mut self.0).poll_next(cx)
    }
}
