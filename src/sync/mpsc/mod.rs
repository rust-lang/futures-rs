//! A multi-producer, single-consumer, futures-aware, FIFO queue with back pressure.
//!
//! A channel can be used as a communication primitive between tasks running on
//! `futures-rs` executors. Channel creation provides `Receiver` and `Sender`
//! handles. `Receiver` implements `Stream` and allows a task to read values
//! out of the channel. If there is no message to read from the channel, the
//! current task will be notified when a new value is sent. `Sender` implements
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

use std::fmt;
use std::error::Error;
use std::any::Any;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
use std::thread;
use std::usize;
use std::isize;
use std::boxed::Box;

use sync::mpsc::queue::{Queue, PopResult};
use task::{self, Task};
use {Async, AsyncSink, Poll, StartSend, Sink, Stream};

mod atomic_box_option;
mod queue;

/// The transmission end of a channel which is used to send values.
///
/// This is created by the `channel` method.
#[derive(Debug)]
pub struct Sender<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<BoundedInner<T>>,

    // Handle to the task that is blocked on this sender. This handle is sent
    // to the receiver half in order to be notified when the sender becomes
    // unblocked.
    sender_task: SenderTask,

    // True if the sender might be blocked. This is an optimization to avoid
    // having to lock the mutex most of the time.
    maybe_parked: bool,
}

/// The transmission end of a channel which is used to send values.
///
/// This is created by the `unbounded` method.
#[derive(Debug)]
pub struct UnboundedSender<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<UnboundedInner<T>>,
}

trait AssertKinds: Send + Sync + Clone {}
impl AssertKinds for UnboundedSender<u32> {}


/// The receiving end of a channel which implements the `Stream` trait.
///
/// This is a concrete implementation of a stream which can be used to represent
/// a stream of values being computed elsewhere. This is created by the
/// `channel` method.
#[derive(Debug)]
pub struct Receiver<T> {
    inner: Arc<BoundedInner<T>>,
}

/// The receiving end of a channel which implements the `Stream` trait.
///
/// This is a concrete implementation of a stream which can be used to represent
/// a stream of values being computed elsewhere. This is created by the
/// `unbounded` method.
#[derive(Debug)]
pub struct UnboundedReceiver<T> {
    inner: Arc<UnboundedInner<T>>,
}

/// Error type for sending, used when the receiving end of a channel is
/// dropped
#[derive(Clone, PartialEq, Eq)]
pub struct SendError<T>(T);

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("SendError")
            .field(&"...")
            .finish()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "send failed because receiver is gone")
    }
}

impl<T: Any> Error for SendError<T>
{
    fn description(&self) -> &str {
        "send failed because receiver is gone"
    }
}

impl<T> SendError<T> {
    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.0
    }
}

#[derive(Debug)]
struct UnboundedInner<T> {
    task: atomic_box_option::AtomicBoxOption<Task>,
    // `0` if closed, otherwise number of senders
    open_state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<T>,
}

#[derive(Debug)]
struct BoundedInner<T> {
    // Max buffer size of the channel.
    buffer: usize,

    // Internal channel state. Consists of the number of messages stored in the channel.
    num_messages: AtomicUsize,

    common: UnboundedInner<T>,

    // Atomic, FIFO queue used to send parked task handles to the receiver.
    parked_queue: Queue<SenderTask>,
}

// The maximum number of messages that a channel can track is `isize::MAX`
const MAX_CAPACITY: usize = isize::MAX as usize;

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

    let inner = Arc::new(BoundedInner {
        buffer: buffer,
        num_messages: AtomicUsize::new(0),
        common: UnboundedInner {
            task: atomic_box_option::AtomicBoxOption::new(),
            open_state: AtomicUsize::new(1),
            message_queue: Queue::new(),
        },
        parked_queue: Queue::new(),
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
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let inner = Arc::new(UnboundedInner {
        task: atomic_box_option::AtomicBoxOption::new(),
        open_state: AtomicUsize::new(1),
        message_queue: Queue::new(),
    });

    let tx = UnboundedSender {
        inner: inner.clone(),
    };

    let rx = UnboundedReceiver {
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
    // Do the send without failing
    // None means close
    fn do_send(&mut self, msg: T) -> Result<(), SendError<T>> {
        if !self.inner.common.is_open() {
            return Err(SendError(msg));
        }

        // First, increment the number of messages contained by the channel.
        // This operation will also atomically determine if the sender task
        // should be parked.
        //
        // None is returned in the case that the channel has been closed by the
        // receiver. This happens when `Receiver::close` is called or the
        // receiver is dropped.
        let park_self = self.inc_num_messages();

        // If the channel has reached capacity, then the sender task needs to
        // be parked. This will send the task handle on the parked task queue.
        //
        // However, when `do_send` is called while dropping the `Sender`,
        // `task::current()` can't be called safely. In this case, in order to
        // maintain internal consistency, a blank message is pushed onto the
        // parked task queue.
        if park_self {
            self.park();
        }

        // Push the message onto the message queue
        self.inner.common.message_queue.push(msg);

        // Signal to the receiver that a message has been enqueued. If the
        // receiver is parked, this will unpark the task.
        self.inner.common.receiver_notify();

        Ok(())
    }

    // Increment the number of queued messages. Returns if the sender should
    // block.
    fn inc_num_messages(&self) -> bool {
        let mut curr = self.inner.num_messages.load(SeqCst);

        loop {
            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(curr < MAX_CAPACITY, "buffer space exhausted; \
                    sending this messages would overflow the state");

            let next = curr + 1;
            match self.inner.num_messages.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => {
                    // Block if the current number of pending messages has exceeded
                    // the configured buffer size
                    return next > self.inner.buffer;
                }
                Err(actual) => curr = actual,
            }
        }
    }

    fn park(&mut self) {
        // TODO: clean up internal state if the task::current will fail

        let task = Some(task::current());

        *self.sender_task.lock().unwrap() = task;

        // Send handle over queue
        let t = self.sender_task.clone();
        self.inner.parked_queue.push(t);

        // Check to make sure we weren't closed after we sent our task on the
        // queue
        self.maybe_parked = self.inner.common.is_open();
    }

    fn poll_unparked(&mut self) -> Async<()> {
        // First check the `maybe_parked` variable. This avoids acquiring the
        // lock in most cases
        if self.maybe_parked {
            // Get a lock on the task handle
            let mut task = self.sender_task.lock().unwrap();

            if task.is_none() {
                self.maybe_parked = false;
                return Async::Ready(())
            }

            // At this point, an unpark request is pending, so there will be an
            // unpark sometime in the future. We just need to make sure that
            // the correct task will be notified.
            //
            // Update the task in case the `Sender` has been moved to another
            // task
            *task = Some(task::current());

            Async::NotReady
        } else {
            Async::Ready(())
        }
    }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        // If the sender is currently blocked, reject the message before doing
        // any work.
        if !self.poll_unparked().is_ready() {
            return Ok(AsyncSink::NotReady(msg));
        }

        // The channel has capacity to accept the message, so send it.
        try!(self.do_send(msg));

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<T> UnboundedSender<T> {

    /// Sends the provided message along this channel.
    ///
    /// This is an unbounded sender, so this function differs from `Sink::send`
    /// by ensuring the return type reflects that the channel is always ready to
    /// receive messages.
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        if !self.inner.is_open() {
            return Err(SendError(msg));
        }

        self.inner.message_queue.push(msg);

        self.inner.receiver_notify();

        Ok(())
    }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        // The channel has capacity to accept the message, so send it.
        try!(UnboundedSender::send(self, msg));

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<'a, T> Sink for &'a UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        try!(UnboundedSender::send(self, msg));
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> UnboundedSender<T> {
        self.inner.inc_senders();
        
        UnboundedSender {
            inner: self.inner.clone(),
        }
    }
}


impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        assert!(self.inner.common.open_state.load(SeqCst) < self.inner.max_senders());

        self.inner.common.inc_senders();

        Sender {
            inner: self.inner.clone(),
            sender_task: Arc::new(Mutex::new(None)),
            maybe_parked: false,
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.inner.common.dec_senders();
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        self.inner.dec_senders();
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
        self.inner.common.close();

        // Wake up any threads waiting as they'll see that we've closed the
        // channel and will continue on their merry way.
        loop {
            match unsafe { self.inner.parked_queue.pop() } {
                PopResult::Data(task) => {
                    let task = task.lock().unwrap().take();
                    if let Some(task) = task {
                        task.notify();
                    }
                }
                PopResult::Empty => break,
                PopResult::Inconsistent => thread::yield_now(),
            }
        }
    }

    // Unpark a single task handle if there is one pending in the parked queue
    fn unpark_one(&mut self) {
        loop {
            match unsafe { self.inner.parked_queue.pop() } {
                PopResult::Data(task) => {
                    // Do this step first so that the lock is dropped when
                    // `unpark` is called
                    let task = task.lock().unwrap().take();

                    if let Some(task) = task {
                        task.notify();
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

    fn dec_num_messages(&self) {
        let mut curr = self.inner.num_messages.load(SeqCst);

        loop {
            let next = curr - 1;
            match self.inner.num_messages.compare_exchange(curr, next, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(actual) => curr = actual,
            }
        }
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        // Try to read a message off of the message queue.
        let msg = self.inner.common.poll_queue_and_open();

        if let Ok(Async::Ready(Some(_))) = msg {
            // If there are any parked task handles in the parked queue, pop
            // one and unpark it.
            self.unpark_one();

            // Decrement number of messages
            self.dec_num_messages();
        }

        // Return the message
        msg
    }
}




impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        self.inner.common.drain_queue();
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        self.inner.drain_queue();
    }
}

impl<T> UnboundedReceiver<T> {
    /// Closes the receiving half
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        self.inner.close();
    }
}

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        self.inner.poll_queue_and_open()
    }
}

/*
 *
 * ===== impl Inner =====
 *
 */

impl<T> BoundedInner<T> {
    // The return value is such that the total number of messages that can be
    // enqueued into the channel will never exceed MAX_CAPACITY
    fn max_senders(&self) -> usize {
        MAX_CAPACITY - self.buffer
    }
}


impl<T> UnboundedInner<T> {
    fn receiver_notify(&self) {
        if !self.task.load_is_some(SeqCst) {
            return;
        }

        if let Some(task) = self.task.swap_null(SeqCst) {
            task.notify();
        }
    }

    fn receiver_park(&self) {
        self.task.swap_box(Box::new(task::current()), SeqCst);
    }

    fn is_open(&self) -> bool {
        self.open_state.load(SeqCst) != 0
    }

    fn close(&self) {
        self.open_state.store(0, SeqCst);
        self.receiver_notify();
    }

    fn inc_senders(&self) {
        loop {
            let open_state = self.open_state.load(SeqCst);
            if open_state == 0 {
                // Signal is closed, let it remain that way.
                break;
            }
            if let Ok(_) = self.open_state.compare_exchange(
                open_state, open_state + 1, SeqCst, Relaxed)
            {
                break;
            }
        }
    }

    fn dec_senders(&self) {
        loop {
            let open_state = self.open_state.load(SeqCst);
            if open_state == 0 {
                break;
            }

            if let Ok(_) = self.open_state.compare_exchange(
                open_state, open_state - 1, SeqCst, Relaxed)
            {
                if open_state == 1 {
                    // Notify receiver of sender death
                    // so receiver gets EOF
                    self.receiver_notify();
                }

                break;
            }
        }
    }

    fn queue_pop_spin(&self) -> Async<T> {
        // Pop off a message
        loop {
            match unsafe { self.message_queue.pop() } {
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
                    // 3) task::current().unwrap() & return NotReady
                    //
                    // For now, thread::yield_now() is used, but it would
                    // probably be better to spin a few times then yield.
                    thread::yield_now();
                }
            }
        }
    }

    /// Poll queue and open flag together.
    fn poll_queue_and_open(&self) -> Poll<Option<T>, ()> {
        // Try to read a message off of the message queue.
        return Ok(match self.queue_pop_spin() {
            Async::Ready(msg) => Async::Ready(Some(msg)),
            Async::NotReady => {

                // Park only if open, otherwise it will be unnecessary work.
                if self.is_open() {

                    // AT THIS POINT other thread may call `notify`,
                    // so we need to check for `is_open` again later.

                    self.receiver_park();
                }

                // It is possible that channel is closed before we parked
                // so we need to check for openness again.
                // Because nobody is going to call `notify`.

                // Call for `is_open` must come before queue pop,
                // because channel is closed after queue updated.
                let open = self.is_open();

                match self.queue_pop_spin() {
                    Async::NotReady => {
                        if open {
                            Async::NotReady
                        } else {
                            Async::Ready(None)
                        }
                    },
                    Async::Ready(msg) => Async::Ready(Some(msg)),
                }
            }
        });
    }

    /// Drain queue on receiver drop to release memory early
    fn drain_queue(&self) {
        while self.queue_pop_spin().is_ready() {
        }
    }
}
