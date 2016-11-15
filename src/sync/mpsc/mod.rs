//! A multi-producer, single-consumer, futures-aware, FIFO queue with back pressure.
//!
//! A channel can be used as a communication primitive between tasks running on
//! `futures-rs` executors. Channel creation provides `Receiver` and `Sender`
//! handles. `Receiver` implements `Stream` and allows a task to read values
//! out of the channel. If there is no message to read from the channel, the
//! curernt task will be notified when a new value is sent. `Sender` implements
//! the `Sink` trait and allows a task to send messages into the channel. If
//! the channel is at capacity, then send will be rejected and the task will be
//! notified when additional capacity is available.
//!
//! # Disconnection
//!
//! When all `Sender` handles have been dropped, it is no longer possible to
//! send values into the channel. This is considered the termination event of
//! the stream. As such, `Sender::poll` will return `Ok(Ready(None))`.
//!
//! If the receiver handle is dropped, then messages can no longer be read out
//! of the channel. In this case, a `send` will result in an error.
//!
//! # Clean Shutdown
//!
//! If the `Receiver` is simply dropped, then it is possible for there to be
//! messages still in the channel that will not be processed. As such, it is
//! usually desirable to perform a "clean" shutdown. To do this, the receiver
//! will first call `close`, which will prevent any further messages to be sent
//! into the channel. Then, the receiver consumes the channel to completion, at
//! which point the receiver can be dropped.

// At the core, the channel uses an atomic FIFO queue for message passing. This
// queue is used as the primary coordination primitive. In order to enforce
// capacity limits and handle back pressure, a secondary FIFO queue is used to
// send parked task handles.
//
// The general idea is that the channel is created with a `buffer` size of `n`.
// The channel capacity is `n + num-senders`. Each sender gets one "guaranteed"
// slot to hold a message. This allows `Sender` to know for a fact that a send
// will succeed *before* starting to do the actual work of sending the value.
// Since most of this work is lock-free, once the work starts, it is impossible
// to safely revert.
//
// If the sender is unable to process a send operation, then the the curren
// task is parked and the handle is sent on the parked task queue.
//
// Note that the implementation guarantees that the channel capacity will never
// exceed the configured limit, however there is no *strict* guarantee that the
// receiver will wake up a parked task *immediately* when a slot becomes
// available. However, it will almost always unpark a task when a slot becomes
// available and it is *guaranteed* that a sender will be unparked when the
// message that caused the sender to become parked is read out of the channel.
//
// The steps for sending a message are roughly:
//
// 1) Increment the channel message count
// 2) If the channel is at capacity, push the task handle onto the wait queue
// 3) Push the message onto the message queue.
//
// The steps for receiving a message are roughly:
//
// 1) Pop a message from the message queue
// 2) Pop a task handle from the wait queue
// 3) Decrement the channel message count.
//
// It's important for the order of operations on lock-free structures to happen
// in reverse order between the sender and receiver. This makes the message
// queue the primary coordination structure and establishes the necessary
// happens-before semantics required for the acquire / release semantics used
// by the queue structure.

mod queue;

use self::queue::{Queue, PopResult};

use {Async, AsyncSink, Poll, StartSend};
use task::{self, Task};
use sink::{Sink};
use stream::Stream;

use std::{thread, usize};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

pub use std::sync::mpsc::SendError;

/// The transmission end of a channel which is used to send values.
///
/// This is created by the `channel` method.
pub struct Sender<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<Inner<T>>,

    // Handle to the task that is blocked on this sender. This handle is sent
    // to the receiver half in order to be notified when the sender becomes
    // unblocked.
    sender_task: SenderTask,

    // True if the sender might be blocked. This is an optimization to avoid
    // having to lock the mutex most of the time.
    maybe_parked: bool,
}

/// The receiving end of a channel which implements the `Stream` trait.
///
/// This is a concrete implementation of a stream which can be used to represent
/// a stream of values being computed elsewhere. This is created by the
/// `channel` method.
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    // Max buffer size of the channel. If `None` then the channel is unbounded.
    buffer: Option<usize>,

    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<Option<T>>,

    // Atomic, FIFO queue used to send parked task handles to the receiver.
    parked_queue: Queue<SenderTask>,

    // Number of senders in existence
    num_senders: AtomicUsize,

    // Handle to the receiver's task.
    recv_task: Mutex<Option<Task>>,
}

// Struct representation of `Inner::state`.
#[derive(Debug, Clone, Copy)]
struct State {
    // `true` when the channel is open
    is_open: bool,

    // Number of messages in the channel
    num_messages: usize,
}

// Returned from Receiver::try_park()
enum TryPark {
    Parked,
    Closed,
    NotEmpty,
}

// The `is_open` flag is stored in the left-most bit of `Inner::state`
const OPEN_MASK: usize = 1 << 31;

// When a new channel is created, it is created in the open state with no
// pending messages.
const INIT_STATE: usize = OPEN_MASK;

// The maximum number of messages that a channel can track is `usize::MAX > 1`
const MAX_CAPACITY: usize = !(OPEN_MASK);

// The maximum requested buffer size must be less than the maximum capacity of
// a channel. This is because each sender gets a guaranteed slot.
const MAX_BUFFER: usize = MAX_CAPACITY >> 1;

// Sent to the consumer to wake up blocked producers
type SenderTask = Arc<Mutex<Option<Task>>>;

/// Creates an in-memory channel implementation of the `Stream` trait with
/// bounded capacity.
///
/// This method creates a concrete implementation of the `Stream` trait which
/// can be used to send values across threads in a streaming fashion. This
/// channel is unique in that it implements back pressure to ensure that the
/// sender never outpaces the receiver. The channel capacity is equal to
/// `buffer + num-senders`. In other words, each sender gets a guaranteed slot
/// in the channel capacity, and on top of that there are `buffer` "first come,
/// first serve" slots available to all senders.
///
/// The `Receiver` returned implements the `Stream` trait and has access to any
/// number of the associated combinators for transforming the result.
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    // Check that the requested buffer size does not exceed the maximum buffer
    // size permitted by the system.
    assert!(buffer < MAX_BUFFER, "requested buffer size too large");
    channel2(Some(buffer))
}

/// Creates an in-memory channel implementation of the `Stream` trait with
/// unbounded capacity.
///
/// This method creates a concrete implementation of the `Stream` trait which
/// can be used to send values across threads in a streaming fashion. A `send`
/// on this channel will always succeed as long as the receive half has not
/// been closed. If the receiver falls behind, messages will be buffered
/// internally.
///
/// **Note** that the amount of available system memory is an implicit bound to
/// the channel. Using an `unbounded` channel has the ability of causing the
/// process to run out of memory. In this case, the process will be aborted.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    // usize::MAX is a special case where producers will never be blocked
    channel2(None)
}

fn channel2<T>(buffer: Option<usize>) -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        buffer: buffer,
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        parked_queue: Queue::new(),
        num_senders: AtomicUsize::new(1),
        recv_task: Mutex::new(None),
    });

    let tx = Sender {
        inner: inner.clone(),
        sender_task: Arc::new(Mutex::new(None)),
        maybe_parked: false,
    };

    let rx = Receiver {
        inner: inner,
    };

    (tx, rx)
}

/*
 *
 * ===== impl Sender =====
 *
 */

impl<T> Sender<T> {
    fn try_clone(&self) -> Option<Sender<T>> {
        // Since this atomic op isn't actually guarding any memory and we don't
        // care about any orderings besides the ordering on the single atomic
        // variable, a relaxed ordering is acceptable.
        let mut curr = self.inner.num_senders.load(Ordering::Relaxed);

        loop {
            // If the maximum number of senders has been reached, then fail
            if curr == self.inner.max_senders() {
                return None;
            }

            debug_assert!(curr < self.inner.max_senders());

            let next = curr + 1;
            let actual = self.inner.num_senders.compare_and_swap(curr, next, Ordering::Relaxed);

            // The ABA problem doesn't matter here. We only care that the
            // number of senders never exceeds the maximum.
            if actual == curr {
                return Some(Sender {
                    inner: self.inner.clone(),
                    sender_task: Arc::new(Mutex::new(None)),
                    maybe_parked: false,
                });
            }

            curr = actual;
        }
    }

    /// Returns `Async::Ready(())` if sending a message will succeed
    ///
    /// If `Async::NotReady` is returned, the current task will be notified
    /// once the `Sender` becomes ready to accept a new value.
    pub fn poll_ready(&mut self) -> Async<()> {
        // First check the `maybe_parked` variable. This avoids acquiring the
        // lock in most cases
        if self.maybe_parked {
            // Get a lock on the task handle
            let mut task = self.sender_task.lock().unwrap();

            if task.is_none() {
                self.maybe_parked = false;
                return Async::Ready(());
            }

            // At this point, an unpark request is pending, so there will be an
            // unpark sometime in the future. We just need to make sure that
            // the correct task will be notified.
            //
            // Update the task in case the `Sender` has been moved to another
            // task
            *task = Some(task::park());

            Async::NotReady
        } else {
            Async::Ready(())
        }
    }

    fn start_send2(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        // If the sender is currently blocked, reject the message before doing
        // any work.
        if !self.poll_ready().is_ready() {
            return Ok(AsyncSink::NotReady(msg));
        }

        // The channel has capacity to accept the message, so send it.
        try!(self.do_send(Some(msg), true));

        Ok(AsyncSink::Ready)
    }

    // Do the send without failing
    fn do_send(&mut self, msg: Option<T>, can_park: bool) -> Result<(), SendError<T>> {
        // First, increment the number of messages contained by the channel.
        // This operation will also atomically determine if the sender task
        // should be parked and if the receiver task should be unparked.
        //
        // None is returned in the case that the channel has been closed by the
        // receiver. This happens when `Receiver::close` is called or the
        // receiver is dropped.
        let (park_self, unpark_recv) = match self.inc_num_messages(msg.is_none()) {
            Some((park_self, unpark_recv)) => (park_self, unpark_recv),
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
                    return Err(SendError(msg));
                } else {
                    return Ok(());
                }
            }
        };

        // If the channel has reached capacity, then the sender task needs to
        // be parked. This will send the task handle on the parked task queue.
        //
        // However, when `do_send` is called while dropping the `Sender`,
        // `task::park()` can't be called safely. In this case, in order to
        // maintain internal consistency, a blank message is pushed onto the
        // parked task queue.
        if park_self {
            self.park(can_park);
        }

        // Push the message onto the message queue
        self.inner.message_queue.push(msg);

        // If sending the message caused the number of messages contained by
        // the channel to go from 0 -> 1, then the sender is responsible to
        // unpark the receiver (if it is currently parked).
        if unpark_recv {
            // Do this step first so that the lock is dropped when
            // `unpark` is called
            let task = self.inner.recv_task.lock().unwrap().take();

            if let Some(task) = task {
                task.unpark();
            }
        }

        Ok(())
    }

    // Increment the number of queued messages. Returns if the sender should
    // block.
    fn inc_num_messages(&self, close: bool) -> Option<(bool, bool)> {
        let mut curr = self.inner.state.load(Ordering::SeqCst);

        loop {
            let mut state = decode_state(curr);

            // The receiver end closed the channel.
            if !state.is_open {
                return None;
            }

            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(state.num_messages < MAX_CAPACITY, "buffer space exhausted; sending this messages would overflow the state");

            state.num_messages += 1;

            // The channel is closed by all sender handles being dropped.
            if close {
                state.is_open = false;
            }

            let next = encode_state(&state);
            let actual = self.inner.state.compare_and_swap(curr, next, Ordering::SeqCst);

            if curr == actual {
                // Block if the current number of pending messages has exceeded
                // the configured buffer size
                let park_self = match self.inner.buffer {
                    Some(buffer) => state.num_messages > buffer,
                    None => false,
                };

                // Only unpark the receive half if transitioning from 0 -> 1.
                let unpark_recv = state.num_messages == 1;

                return Some((park_self, unpark_recv));
            }

            curr = actual;
        }
    }

    fn park(&mut self, can_park: bool) {
        // TODO: clean up internal state if the task::park will fail

        let task = if can_park {
            Some(task::park())
        } else {
            None
        };

        self.maybe_parked = true;
        *self.sender_task.lock().unwrap() = task;

        // Send handle over queue
        let t = self.sender_task.clone();
        self.inner.parked_queue.push(t);
    }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        self.start_send2(msg)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        self.try_clone().expect("failed to clone sender; maximum number of senders already reached")
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // Ordering between variables don't matter here
        let prev = self.inner.num_senders.fetch_sub(1, Ordering::Relaxed);

        if prev == 1 {
            let _ = self.do_send(None, false);
        }
    }
}

/*
 *
 * ===== impl Receiver =====
 *
 */

impl<T> Receiver<T> {
    /// Closes the receiving half
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        // A relaxed memory ordering is acceptable given that toggling the
        // flag is an isolated operation. If no further functions are
        // called on `Receiver` then the outcome of this function doesn't
        // really matter. If `poll` is called after this, then the the same
        // cell will be operated on again with stronger ordering.
        let mut curr = self.inner.state.load(Ordering::Relaxed);

        loop {
            let mut state = decode_state(curr);

            if !state.is_open {
                return;
            }

            state.is_open = false;

            let next = encode_state(&state);
            let actual = self.inner.state.compare_and_swap(curr, next, Ordering::Relaxed);

            if actual == curr {
                return;
            }

            curr = actual;
        }
    }

    fn next_message(&self) -> Async<Option<T>> {
        // Pop off a message
        loop {
            match unsafe { self.inner.message_queue.pop() } {
                PopResult::Data(msg) => {
                    return Async::Ready(msg);
                }
                PopResult::Empty => {
                    // The queue is empty, return NotReady
                    return Async::NotReady;
                }
                PopResult::Inconsistent => {
                    // Inconsistent means that there will be a message to pop
                    // in a short time. This branch can only be reached if
                    // values are being produced from another thread, so there
                    // are a few ways that we can deal with this:
                    //
                    // 1) Spin
                    // 2) thread::yield_now()
                    // 3) task::park().unwrap() & return NotReady
                    //
                    // For now, thread::yield_now() is used, but it would
                    // probably be better to spin a few times then yield.
                    thread::yield_now();
                }
            }
        }
    }

    // Unpark a single task handle if there is one pending in the parked queue
    fn unpark_one(&self) {
        loop {
            match unsafe { self.inner.parked_queue.pop() } {
                PopResult::Data(task) => {
                    // Do this step first so that the lock is dropped when
                    // `unpark` is called
                    let task = task.lock().unwrap().take();

                    if let Some(task) = task {
                        task.unpark();
                    }

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
    fn try_park(&self) -> TryPark {
        let curr = self.inner.state.load(Ordering::SeqCst);
        let state = decode_state(curr);

        // If there are pending messages, then there is no need to park.
        if state.num_messages > 0 {
            return TryPark::NotEmpty;
        }

        // If the channel is closed, then there is no need to park.
        if !state.is_open {
            return TryPark::Closed;
        }

        // First, track the task in the `recv_task` slot
        let mut task = self.inner.recv_task.lock().unwrap();
        *task = Some(task::park());

        // Ensure that there are still no messages. It is possible for a
        // message to have been sent while updating the task slot.
        let curr = self.inner.state.load(Ordering::SeqCst);
        let state = decode_state(curr);

        if state.num_messages == 0 {
            TryPark::NotEmpty
        } else {
            TryPark::Parked
        }
    }

    fn dec_num_messages(&self) {
        let mut curr = self.inner.state.load(Ordering::SeqCst);

        loop {
            let mut state = decode_state(curr);

            state.num_messages -= 1;

            let next = encode_state(&state);
            let actual = self.inner.state.compare_and_swap(curr, next, Ordering::SeqCst);

            if actual == curr {
                return;
            }

            curr = actual;
        }
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        loop {
            // Try to read a message off of the message queue.
            let msg = match self.next_message() {
                Async::Ready(msg) => msg,
                Async::NotReady => {
                    // There are no messages to read, in this case, attempt to
                    // park. The act of parking will verify that the channel is
                    // still empty after the park operation has completed.
                    match self.try_park() {
                        TryPark::Parked => {
                            // The task was parked, and the channel is still
                            // empty, return NotReady.
                            return Ok(Async::NotReady);
                        }
                        TryPark::Closed => {
                            // The channel is closed, there will be no further
                            // messages.
                            return Ok(Async::Ready(None));
                        }
                        TryPark::NotEmpty => {
                            // A message was sent while trying to park. Yield
                            // the thread and try the poll operation again.
                            thread::yield_now();
                            continue;
                        }
                    }
                }
            };

            // If there are any parked task handles in the parked queue, pop
            // one and unpark it.
            self.unpark_one();

            // Decrement number of messages
            self.dec_num_messages();

            // Return the message
            return Ok(Async::Ready(msg));
        }
    }
}

/*
 *
 * ===== impl Inner =====
 *
 */

impl<T> Inner<T> {
    // The return value is such that the total number of messages that can be
    // enqueued into the channel will never exceed MAX_CAPACITY
    fn max_senders(&self) -> usize {
        match self.buffer {
            Some(buffer) => MAX_CAPACITY - buffer,
            None => MAX_BUFFER,
        }
    }
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

/*
 *
 * ===== Helpers =====
 *
 */

fn decode_state(num: usize) -> State {
    State {
        is_open: num & OPEN_MASK == OPEN_MASK,
        num_messages: num & MAX_CAPACITY,
    }
}

fn encode_state(state: &State) -> usize {
    let mut num = state.num_messages;

    if state.is_open {
        num |= OPEN_MASK;
    }

    num
}
