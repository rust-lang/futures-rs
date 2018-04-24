//! A multi-producer, single-consumer queue for sending values across
//! asynchronous tasks.
//!
//! Similarly to the `std`, channel creation provides [`Receiver`](Receiver) and
//! [`Sender`](Sender) handles. [`Receiver`](Receiver) implements
//! [`Stream`](futures_core::Stream) and allows a task to read values out of the
//! channel. If there is no message to read from the channel, the current task
//! will be awoken when a new value is sent. [`Sender`](Sender) implements the
//! `Sink` trait and allows a task to send messages into
//! the channel. If the channel is at capacity, the send will be rejected and
//! the task will be awoken when additional capacity is available. This process
//! of delaying sends beyond a certain capacity is often referred to as
//! "backpressure".
//!
//! Unbounded channels (channels without backpressure) are also available using
//! the [`unbounded`](unbounded) function.
//!
//! # Disconnection
//!
//! When all [`Sender`](Sender)s have been dropped, it is no longer
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
// send wakers for blocked Sender tasks.
//
// The general idea is that the channel is created with a `buffer` size of `n`.
// The channel capacity is limited to `n`.
//
// When a sender tries to send (via `poll_ready_helper`) it will attempt to
// increment the current number of "send reservations". If incrementing this
// count would result in `> n` reservations, the `Sender` task is scheduled
// to receive a wakeup when the number of "send reservations" drops.
//
// Note that the implementation guarantees that the channel capacity will never
// exceed the configured limit, however there is no *strict* guarantee that the
// receiver will wake up a `Sender` task *immediately* when a slot becomes
// available. However, it will always awaken a `Sender` if one is blocked when
// a message is received.

use std::fmt;
use std::error::Error;
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::atomic::Ordering::{Relaxed, SeqCst};
use std::sync::{Arc, Mutex};
use std::thread;
use std::usize;

use futures_core::task::{self, Waker, AtomicWaker};
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

    // Waker for to the task that is blocked on this sender.
    // This handle is sent to the receiver half in order to be notified when
    // the sender becomes unblocked.
    //
    // When the sender is dropped, it sets the waker to `None` so that the
    // receiver knows to wake up a different sender instead.
    sender_waker: Arc<Mutex<SenderWaker>>,

    // Cached value of `permission_to_send` in `SenderWaker` above.
    // If `permission_to_send_cached` is true, then `permission_to_send`
    // must also be true. However, `permission_to_send` may be true even
    // when `permission_to_send_cached` is false.
    permission_to_send_cached: bool,
}

#[derive(Debug)]
struct SenderWaker {
    /// An optional task to wake. `None` if the `Sender` has been dropped
    /// or has 
    waker: Option<Waker>,

    /// Whether or not the permission to send has already been acquired.
    /// If this is false, then the sender must compare-exchange-increment
    /// the number of cueued messages in `Inner` before it can be considered
    /// "ready" to send a message. Once a message is sent, this goes back to
    /// false. If the Sender is dropped while this flag is true, then the
    /// number of cueued messages in `Inner` must be decremented so that other
    /// `Sender`s can send.
    ///
    /// If `permission_to_send` is true, `waker` should be `None`.
    permission_to_send: bool,
}

impl SenderWaker {
    fn new() -> Self {
        SenderWaker { waker: None, permission_to_send: false }
    }
}

/// The receiving end of a bounded mpsc channel.
///
/// This value is created by the [`channel`](channel) function.
#[derive(Debug)]
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

#[derive(Debug)]
struct Inner<T> {
    // Max buffer size of the channel. If `None` then the channel is unbounded.
    buffer: Option<usize>,

    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<T>,

    // Atomic, FIFO queue used to send wakers for blocked `Sender` tasks to the `Receiver`.
    sender_waker_queue: Queue<Arc<Mutex<SenderWaker>>>,
    // Lock for popping off of the sender_waker_queue
    sender_waker_pop_lock: Mutex<()>,

    // Waker for the receiver's task.
    recv_waker: AtomicWaker,
}

// Struct representation of `Inner::state`.
#[derive(Debug, Clone, Copy)]
struct State {
    // `true` when the channel is open
    is_open: bool,

    // Number of messages in the channel
    send_reservations: usize,
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

/// Creates a bounded mpsc channel for communicating between asynchronous tasks.
///
/// Being bounded, this channel provides backpressure to ensure that the sender
/// outpaces the receiver by only a limited amount. The channel's capacity is
/// equal to `buffer + 1`. That is, there can be `buffer + 1` number of
/// messages in-flight before the channel will start providing backpressure.
///
/// The [`Receiver`](Receiver) returned implements the
/// [`Stream`](futures_core::Stream) trait, while [`Sender`](Sender) implements
/// `Sink`.
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    // Check that the requested buffer size does not exceed the maximum buffer
    // size permitted by the system.
    assert!(buffer < MAX_BUFFER, "requested buffer size too large");
    channel2(Some(buffer + 1))
}

fn channel2<T>(buffer: Option<usize>) -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        buffer: buffer,
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        sender_waker_queue: Queue::new(),
        sender_waker_pop_lock: Mutex::new(()),
        recv_waker: AtomicWaker::new(),
    });

    let tx = Sender {
        inner: inner.clone(),
        sender_waker: Arc::new(Mutex::new(SenderWaker::new())),
        permission_to_send_cached: false,
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
        match self.poll_ready_helper(None) {
            Ok(Async::Ready(())) => {},
            Ok(Async::Pending) => unreachable!(),
            Err(e) => return Err(TrySendError {
                err: e,
                val: msg,
            }),
        }

        // The channel has capacity to accept the message, so send it
        if let Err(e) = self.inner.check_open() {
            return Err(TrySendError { err: e, val: msg });
        }
        self.inner.message_queue.push(msg);
        {
            let mut sender_waker_lock = self.sender_waker.lock().unwrap();
            sender_waker_lock.waker = None;
            assert!(sender_waker_lock.permission_to_send);
            sender_waker_lock.permission_to_send = false;
            self.permission_to_send_cached = false;
        }
        self.inner.recv_waker.wake();
        Ok(())
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

    // Increment the number of reservations for sending a message.
    fn inc_send_reservations(&self) -> Result<Async<()>, SendError> {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            // The receiver end closed the channel.
            if !state.is_open {
                return Err(SendError::disconnected());
            }

            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(state.send_reservations < MAX_CAPACITY, "buffer space exhausted; \
                    sending this messages would overflow the state");

            // Return if the buffer is maxed out
            if let Some(max) = self.inner.buffer {
                debug_assert!(max >= state.send_reservations);
                if max == state.send_reservations {
                    return Ok(Async::Pending);
                }
            }

            state.send_reservations += 1;

            let next = encode_state(&state);
            match self.inner.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => return Ok(Async::Ready(())),
                Err(actual) => curr = actual,
            }
        }
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
        self.poll_ready_helper(Some(cx))
    }

    /// A helper for `poll_ready` and `try_send`.
    /// 
    /// `cx_opt`: if `Some`, a full channel will result in the `Waker` in `Context` being
    /// queued for a wakeup when the channel is ready to receive a message. If `None`,
    /// an error is returned rather than queueing the task.
    fn poll_ready_helper(&mut self, cx_opt: Option<&mut task::Context>) -> Poll<(), SendError> {
        // Start off by checking the cached value
        if self.permission_to_send_cached {
            self.inner.check_open()?;
            return Ok(Async::Ready(()));
        }

        // Check the real value behind the lock to see if it was set
        // by the reciever.
        let mut sender_waker_lock = self.sender_waker.lock().unwrap();
        if sender_waker_lock.permission_to_send {
            debug_assert!(sender_waker_lock.waker.is_none());
            self.permission_to_send_cached = true;
            return Ok(Async::Ready(()));
        }

        // Attempt to make a new send reservation.
        if let Async::Ready(()) = self.inc_send_reservations()? {
            self.permission_to_send_cached = true;
            sender_waker_lock.permission_to_send = true;
            sender_waker_lock.waker = None;
            return Ok(Async::Ready(()));
        }

        let waker = if let Some(cx) = cx_opt {
            cx.waker().clone()
        } else {
            return Err(SendError::full());
        };

        // Update the waker and enqueue sender_waker if necessary
        let was_already_queued = sender_waker_lock.waker.is_some();
        sender_waker_lock.waker = Some(waker);
        if !was_already_queued {
            self.inner.sender_waker_queue.push(self.sender_waker.clone());
        }

        // Now that we've queued for a wakeup, try and increment again to make sure
        // we didn't race a decrement.
        if let Async::Ready(()) = self.inc_send_reservations()? {
            self.permission_to_send_cached = true;
            sender_waker_lock.permission_to_send = true;
            sender_waker_lock.waker = None;
            return Ok(Async::Ready(()));
        }

        Ok(Async::Pending)
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        if let Ok(()) = self.inner.check_open() {
            true
        } else {
            false
        }
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&mut self) {
        self.inner.close()
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let had_permission_to_send;
        {
            let mut sender_waker_lock = self.sender_waker.lock().unwrap();
            sender_waker_lock.waker = None;
            had_permission_to_send = sender_waker_lock.permission_to_send;
            sender_waker_lock.permission_to_send = false;
        }

        // If we had already received permission to send a message,
        // release that permission.
        if had_permission_to_send {
            self.inner.release_send_reservation();
        }

        // If the sender is the last once,
        // close the channel and awaken the receiver
        if Arc::strong_count(&self.inner) == 2 {
            self.close_channel();
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            inner: self.inner.clone(),
            sender_waker: Arc::new(Mutex::new(SenderWaker::new())),
            permission_to_send_cached: false,
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
        self.inner.close()
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

    /// Pops off the next message.
    /// Returns `None` if no message is present and the channel is closed.
    fn next_message(&mut self) -> Async<Option<T>> {
        // Pop off a message
        loop {
            // Safe because this is the only place the message queue is popped,
            // and it takes `&mut self` to ensure that only the unique reciever
            // can pop off of the message queue.
            match unsafe { self.inner.message_queue.pop() } {
                PopResult::Data(msg) => {
                    self.inner.release_send_reservation();
                    return Async::Ready(Some(msg));
                }
                PopResult::Empty => {
                    if let Err(_) = self.inner.check_open() {
                        return Async::Ready(None);
                    }

                    // The queue is empty but not closed, return Pending
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
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = Never;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        // Try to read a message off of the message queue.
        let msg = match self.next_message() {
            Async::Ready(msg) => Async::Ready(msg),
            Async::Pending => {
                self.inner.recv_waker.register(cx.waker());
                // Check again for a message to make sure we didn't race.
                self.next_message()
            }
        };

        Ok(msg)
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        while let Async::Ready(Some(_)) = self.next_message() {}
    }
}

/*
 *
 * ===== impl Inner =====
 *
 */

impl<T> Inner<T> {
    /// Release a reservation for sending a message.
    ///
    /// This method must be called when messages are successfully received
    /// and when senders holding reservations are dropped.
    fn release_send_reservation(&self) {
        // If there are any waiting senders, attempt to give them
        // permission to send and awaken them before decrementing the
        // number of send reservations.
        if !self.wake_one(true) {
            // If no sender was awoken and given permissions, decrement
            // the number of send reservations.
            self.dec_send_reservations();

            // Attempt to wake a new sender (without permissions)
            // in case a sender was added to the queue between `wake_one`
            // and the successfull execution of `dec_send_reservations`.
            self.wake_one(false);
        }
    }

    fn pop_sender_waker_queue(&self) -> PopResult<Arc<Mutex<SenderWaker>>> {
        let lock = self.sender_waker_pop_lock.lock().unwrap();
        // Safe becuase we've used the lock above to ensure that only
        // one user at a time can pop.
        let res = unsafe { self.sender_waker_queue.pop() };
        drop(lock);
        res
    }

    /// Wake a single sender.
    ///
    /// `permission`: whether or not to give that task the right to send.
    ///
    /// Returns whether or not a sender was awoken.
    fn wake_one(&self, permission_to_send: bool) -> bool {
        loop {
            match self.pop_sender_waker_queue() {
                PopResult::Data(sender_waker) => {
                    let mut sender_waker = sender_waker.lock().unwrap();
                    if let Some(waker) = sender_waker.waker.take() {
                        // If sender_waker still contains a waker to be awoken,
                        // then it must not yet have been given permission to send.
                        debug_assert!(sender_waker.permission_to_send == false);
                        if permission_to_send {
                            sender_waker.permission_to_send = true;
                        }
                        waker.wake();
                        return true;
                    }
                    // If there was no waker, then the sender no longer
                    // requires a wakeup. Continue on to the next waker in the queue.
                }
                PopResult::Empty => {
                    // Queue empty, no task to wake up.
                    return false;
                }
                PopResult::Inconsistent => {
                    // Same as above
                    thread::yield_now();
                }
            }
        }
    }

    /// Decrease the number of active reservations for sending.
    fn dec_send_reservations(&self) {
        let mut curr = self.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            state.send_reservations -= 1;

            let next = encode_state(&state);
            match self.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(actual) => curr = actual,
            }
        }
    }

    /// Close the channel.
    ///
    /// All senders and the receiver will be woken up.
    fn close(&self) {
        let mut curr = self.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            if !state.is_open {
                break
            }

            state.is_open = false;

            let next = encode_state(&state);
            match self.state.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(actual) => curr = actual,
            }
        }

        // Wake up any senders waiting as they'll see that we've closed the
        // channel and will continue on their merry way.
        loop {
            match self.pop_sender_waker_queue() {
                PopResult::Data(sender_waker) => {
                    if let Some(waker) = sender_waker.lock().unwrap().waker.take() {
                        waker.wake();
                    }
                }
                PopResult::Empty => break,
                // Someone was in the middle of a `push` when we last
                // tried to `pop`.
                PopResult::Inconsistent => thread::yield_now(),
            }
        }

        // Wake up the receiver
        self.recv_waker.wake();
    }

    fn check_open(&self) -> Result<(), SendError> {
        if decode_state(self.state.load(Relaxed)).is_open {
            Ok(())
        } else {
            Err(SendError::disconnected())
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
        send_reservations: num & MAX_CAPACITY,
    }
}

fn encode_state(state: &State) -> usize {
    let mut num = state.send_reservations;

    if state.is_open {
        num |= OPEN_MASK;
    }

    num
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
    let tx = Arc::new(UnboundedInner {
        closed: AtomicBool::new(false),
        message_queue: Queue::new(),
        recv_waker: AtomicWaker::new(),
    });
    let rx = tx.clone();
    (UnboundedSender(tx), UnboundedReceiver(rx))
}

/// The transmission end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`](unbounded) function.
#[derive(Debug, Clone)]
pub struct UnboundedSender<T>(Arc<UnboundedInner<T>>);

/// The receiving end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`](unbounded) function.
#[derive(Debug)]
pub struct UnboundedReceiver<T>(Arc<UnboundedInner<T>>);

trait AssertKinds: Send + Sync + Clone {}
impl AssertKinds for UnboundedSender<u32> {}

#[derive(Debug)]
struct UnboundedInner<T> {
    closed: AtomicBool,
    message_queue: Queue<T>,
    recv_waker: AtomicWaker,
}

impl<T> UnboundedSender<T> {
    /// Check if the channel is ready to receive a message.
    pub fn poll_ready(&self, _: &mut task::Context) -> Poll<(), SendError> {
        Ok(Async::Ready(()))
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        self.0.closed.load(SeqCst)
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&self) {
        self.0.closed.store(true, SeqCst);
        self.0.recv_waker.wake();
    }

    /// Send a message on the channel.
    ///
    /// This method should only be called after `poll_ready` has been used to
    /// verify that the channel is ready to receive a message.
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        if self.0.closed.load(SeqCst) {
            return Err(SendError::disconnected());
        }
        self.0.message_queue.push(msg);
        self.0.recv_waker.wake();
        Ok(())
    }

    /// Sends a message along this channel.
    ///
    /// This is an unbounded sender, so this function differs from `Sink::send`
    /// by ensuring the return type reflects that the channel is always ready to
    /// receive messages.
    pub fn unbounded_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        if self.0.closed.load(SeqCst) {
            return Err(TrySendError {
                err: SendError::disconnected(),
                val: msg,
            });
        }
        self.0.message_queue.push(msg);
        self.0.recv_waker.wake();
        Ok(())
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == 2 { 
            // If it's just us and the reciever, or us and another sender,
            // the channel should be closed.
            self.0.closed.store(true, SeqCst);
            self.0.recv_waker.wake();
        }
    }
}

impl<T> UnboundedReceiver<T> {
    /// Closes the receiving half of the channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        self.0.closed.store(true, SeqCst);
    }

    /// Tries to receive the next message without notifying a context if empty.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only when you've otherwise arranged to be notified when the channel is
    /// no longer empty.
    pub fn try_next(&mut self) -> Result<Option<T>, TryRecvError> {
        loop {
            // Safe because this is the only place the message queue is popped,
            // and it takes `&mut self` to ensure that only the unique reciever
            // can pop off of the message queue.
            match unsafe { self.0.message_queue.pop() } {
                PopResult::Data(msg) => {
                    return Ok(Some(msg));
                }
                PopResult::Empty => {
                    if self.0.closed.load(SeqCst) {
                        return Ok(None);
                    }
                    return Err(TryRecvError { _inner: () });
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
}

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;
    type Error = Never;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        if let Ok(msg) = self.try_next() {
            return Ok(Async::Ready(msg));
        }
        self.0.recv_waker.register(cx.waker());
        if let Ok(msg) = self.try_next() {
            return Ok(Async::Ready(msg));
        }
        Ok(Async::Pending)
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        self.0.closed.store(true, SeqCst);
    }
}

// ----- Error types -----

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

    fn full() -> Self {
        SendError { kind: SendErrorKind::Full }
    }

    fn disconnected() -> Self {
        SendError { kind: SendErrorKind::Disconnected }
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
