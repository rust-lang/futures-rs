//! A multi-producer, single-consumer queue for sending values across
//! asynchronous tasks.
//!
//! Similarly to the `std`, channel creation provides [`Receiver`](Receiver) and
//! [`Sender`](Sender) handles. [`Receiver`](Receiver) implements
//! [`Stream`](futures_core::Stream) and allows a task to read values out of the
//! channel. If there is no message to read from the channel, the current task
//! will be notified when a new value is sent. [`Sender`](Sender) implements the
//! `Sink` trait and allows a task to send messages into
//! the channel. If the channel is at capacity, the send will be rejected and
//! the task will be notified when additional capacity is available. In other
//! words, the channel provides backpressure.
//!
//! Unbounded channels are also available using the [`unbounded`](unbounded)
//! constructor.
//!
//! # Disconnection
//!
//! When all [`Sender`](Sender) handles have been dropped, it is no longer
//! possible to send values into the channel. This is considered the termination
//! event of the stream. As such, [`Receiver::poll_next`](Receiver::poll_next)
//! will return `Ok(Ready(None))`.
//!
//! If the [`Receiver`](Receiver) handle is dropped, then messages can no longer
//! be read out of the channel. In this case, all further attempts to send will
//! result in an error.
//!
//! # Clean Shutdown
//!
//! If the [`Receiver`](Receiver) is simply dropped, then it is possible for
//! there to be messages still in the channel that will not be processed. As
//! such, it is usually desirable to perform a "clean" shutdown. To do this, the
//! receiver will first call `close`, which will prevent any further messages to
//! be sent into the channel. Then, the receiver consumes the channel to
//! completion, at which point the receiver can be dropped.

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
// If the sender is unable to process a send operation, then the current
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

use std::fmt;
use std::error::Error;
use std::any::Any;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::thread;
use std::usize;

use futures_core::task::{self, Waker};
use futures_core::{Async, Poll, Stream};
use futures_core::never::Never;

use mpsc::queue::{Queue, PopResult};

mod queue;

/// The transmission end of a bounded mpsc channel.
///
/// This value is created by the [`channel`](channel) function.
#[derive(Debug)]
pub struct Sender<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<Inner<T>>,

    // Handle to the task that is blocked on this sender. This handle is sent
    // to the receiver half in order to be notified when the sender becomes
    // unblocked.
    sender_task: Arc<Mutex<SenderTask>>,

    // True if the sender might be blocked. This is an optimization to avoid
    // having to lock the mutex most of the time.
    maybe_parked: bool,
}

/// The transmission end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`](unbounded) function.
#[derive(Debug)]
pub struct UnboundedSender<T>(Sender<T>);

trait AssertKinds: Send + Sync + Clone {}
impl AssertKinds for UnboundedSender<u32> {}

/// The receiving end of a bounded mpsc channel.
///
/// This value is created by the [`channel`](channel) function.
#[derive(Debug)]
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

/// The receiving end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`](unbounded) function.
#[derive(Debug)]
pub struct UnboundedReceiver<T>(Receiver<T>);

/// The error type for [`Sender`s](Sender) used as `Sink`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendError {
    kind: SendErrorKind,
}

/// The error type returned from [`try_send`](Sender::try_send).
#[derive(Clone, PartialEq, Eq)]
pub struct TrySendError<T> {
    err: SendError,
    val: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SendErrorKind {
    Full,
    Disconnected,
}

/// The error type returned from [`try_next`](Receiver::try_next).
pub struct TryRecvError {
    _inner: (),
}

impl fmt::Display for SendError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.is_full() {
            write!(fmt, "send failed because channel is full")
        } else {
            write!(fmt, "send failed because receiver is gone")
        }
    }
}

impl Error for SendError {
    fn description(&self) -> &str {
        if self.is_full() {
            "send failed because channel is full"
        } else {
            "send failed because receiver is gone"
        }
    }
}

impl SendError {
    /// Returns true if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        match self.kind {
            SendErrorKind::Full => true,
            _ => false,
        }
    }

    /// Returns true if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        match self.kind {
            SendErrorKind::Disconnected => true,
            _ => false,
        }
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TrySendError")
            .field("kind", &self.err.kind)
            .finish()
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.is_full() {
            write!(fmt, "send failed because channel is full")
        } else {
            write!(fmt, "send failed because receiver is gone")
        }
    }
}

impl<T: Any> Error for TrySendError<T> {
    fn description(&self) -> &str {
        if self.is_full() {
            "send failed because channel is full"
        } else {
            "send failed because receiver is gone"
        }
    }
}

impl<T> TrySendError<T> {
    /// Returns true if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        self.err.is_full()
    }

    /// Returns true if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        self.err.is_disconnected()
    }

    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.val
    }

    /// Drops the message and converts into a `SendError`.
    pub fn into_send_error(self) -> SendError {
        self.err
    }
}

impl fmt::Debug for TryRecvError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("TryRecvError")
            .finish()
    }
}

impl fmt::Display for TryRecvError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(self.description())
    }
}

impl Error for TryRecvError {
    fn description(&self) -> &str {
        "receiver channel is empty"
    }
}

#[derive(Debug)]
struct Inner<T> {
    // Max buffer size of the channel. If `None` then the channel is unbounded.
    buffer: Option<usize>,

    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<Option<T>>,

    // Atomic, FIFO queue used to send parked task handles to the receiver.
    parked_queue: Queue<Arc<Mutex<SenderTask>>>,

    // Number of senders in existence
    num_senders: AtomicUsize,

    // Handle to the receiver's task.
    recv_task: Mutex<ReceiverTask>,
}

// Struct representation of `Inner::state`.
#[derive(Debug, Clone, Copy)]
struct State {
    // `true` when the channel is open
    is_open: bool,

    // Number of messages in the channel
    num_messages: usize,
}

#[derive(Debug)]
struct ReceiverTask {
    unparked: bool,
    task: Option<Waker>,
}

// Returned from Receiver::try_park()
enum TryPark {
    Parked,
    Closed,
    NotEmpty,
}

// The `is_open` flag is stored in the left-most bit of `Inner::state`
const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);

// When a new channel is created, it is created in the open state with no
// pending messages.
const INIT_STATE: usize = OPEN_MASK;

// The maximum number of messages that a channel can track is `usize::MAX >> 1`
const MAX_CAPACITY: usize = !(OPEN_MASK);

// The maximum requested buffer size must be less than the maximum capacity of
// a channel. This is because each sender gets a guaranteed slot.
const MAX_BUFFER: usize = MAX_CAPACITY >> 1;

// Sent to the consumer to wake up blocked producers
#[derive(Debug)]
struct SenderTask {
    task: Option<Waker>,
    is_parked: bool,
}

impl SenderTask {
    fn new() -> Self {
        SenderTask {
            task: None,
            is_parked: false,
        }
    }

    fn notify(&mut self) {
        self.is_parked = false;

        if let Some(task) = self.task.take() {
            task.wake();
        }
    }
}

/// Creates a bounded mpsc channel for communicating between asynchronous tasks.
///
/// Being bounded, this channel provides backpressure to ensure that the sender
/// outpaces the receiver by only a limited amount. The channel's capacity is
/// equal to `buffer + num-senders`. In other words, each sender gets a
/// guaranteed slot in the channel capacity, and on top of that there are
/// `buffer` "first come, first serve" slots available to all senders.
///
/// The [`Receiver`](Receiver) returned implements the
/// [`Stream`](futures_core::Stream) trait, while [`Sender`](Sender) implements
/// `Sink`.
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    // Check that the requested buffer size does not exceed the maximum buffer
    // size permitted by the system.
    assert!(buffer < MAX_BUFFER, "requested buffer size too large");
    channel2(Some(buffer))
}

/// Creates an unbounded mpsc channel for communicating between asynchronous tasks.
///
/// A `send` on this channel will always succeed as long as the receive half has
/// not been closed. If the receiver falls behind, messages will be arbitrarily
/// buffered.
///
/// **Note** that the amount of available system memory is an implicit bound to
/// the channel. Using an `unbounded` channel has the ability of causing the
/// process to run out of memory. In this case, the process will be aborted.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = channel2(None);
    (UnboundedSender(tx), UnboundedReceiver(rx))
}

fn channel2<T>(buffer: Option<usize>) -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        buffer: buffer,
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        parked_queue: Queue::new(),
        num_senders: AtomicUsize::new(1),
        recv_task: Mutex::new(ReceiverTask {
            unparked: false,
            task: None,
        }),
    });

    let tx = Sender {
        inner: inner.clone(),
        sender_task: Arc::new(Mutex::new(SenderTask::new())),
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

    fn poll_ready_nb(&self) -> Poll<(), SendError> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if state.is_open {
            Ok(Async::Ready(()))
        } else {
            Err(SendError {
                kind: SendErrorKind::Full,
            })
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
            assert!(state.num_messages < MAX_CAPACITY, "buffer space exhausted; \
                    sending this messages would overflow the state");

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
    /// capacity, in which case the current task is queued to be notified once capacity is available;
    /// - `Err(SendError)` if the receiver has been dropped.
    pub fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), SendError> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if !state.is_open {
            return Err(SendError {
                kind: SendErrorKind::Disconnected,
            });
        }

        Ok(self.poll_unparked(Some(cx)))
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

    fn poll_unparked(&mut self, cx: Option<&mut task::Context>) -> Async<()> {
        // First check the `maybe_parked` variable. This avoids acquiring the
        // lock in most cases
        if self.maybe_parked {
            // Get a lock on the task handle
            let mut task = self.sender_task.lock().unwrap();

            if !task.is_parked {
                self.maybe_parked = false;
                return Async::Ready(())
            }

            // At this point, an unpark request is pending, so there will be an
            // unpark sometime in the future. We just need to make sure that
            // the correct task will be notified.
            //
            // Update the task in case the `Sender` has been moved to another
            // task
            task.task = cx.map(|cx| cx.waker().clone());

            Async::Pending
        } else {
            Async::Ready(())
        }
    }
}

impl<T> UnboundedSender<T> {
    /// Check if the channel is ready to receive a message.
    pub fn poll_ready(&self, _: &mut task::Context) -> Poll<(), SendError> {
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

/*
 *
 * ===== impl Receiver =====
 *
 */

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
            Async::Ready(msg) => {
                Ok(msg)
            },
            Async::Pending => Err(TryRecvError { _inner: () }),
        }
    }

    fn next_message(&mut self) -> Async<Option<T>> {
        // Pop off a message
        loop {
            match unsafe { self.inner.message_queue.pop() } {
                PopResult::Data(msg) => {
                    // If there are any parked task handles in the parked queue, pop
                    // one and unpark it.
                    self.unpark_one();

                    // Decrement number of messages
                    self.dec_num_messages();

                    return Async::Ready(msg);
                }
                PopResult::Empty => {
                    // The queue is empty, return Pending
                    return Async::Pending;
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

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = Never;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            // Try to read a message off of the message queue.
            let msg = match self.next_message() {
                Async::Ready(msg) => msg,
                Async::Pending => {
                    // There are no messages to read, in this case, attempt to
                    // park. The act of parking will verify that the channel is
                    // still empty after the park operation has completed.
                    match self.try_park(cx) {
                        TryPark::Parked => {
                            // The task was parked, and the channel is still
                            // empty, return Pending.
                            return Ok(Async::Pending);
                        }
                        TryPark::Closed => {
                            // The channel is closed, there will be no further
                            // messages.
                            return Ok(Async::Ready(None));
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
            return Ok(Async::Ready(msg));
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
    type Error = Never;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll_next(cx)
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
