//! A multi-producer, single-consumer, futures-aware, FIFO queue with back
//! pressure, for use communicating between tasks on the same thread.

use std::fmt;
use std::error::Error;
use std::any::Any;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::cell::RefCell;

use task::{self, Task};
use {Async, AsyncSink, Poll, StartSend, Sink, Stream};

/// Creates a bounded in-memory channel with buffered storage.
///
/// This method creates concrete implementations of the `Stream` and `Sink`
/// traits which can be used to communicate a stream of values between tasks
/// with backpressure. The channel capacity is exactly `buffer`. On average,
/// sending a message through this channel performs no dynamic allocation.
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) { channel_(Some(buffer)) }

fn channel_<T>(buffer: Option<usize>) -> (Sender<T>, Receiver<T>) {
    let shared = Rc::new(RefCell::new(Shared {
        buffer: VecDeque::new(),
        capacity: buffer,
        blocked_senders: VecDeque::new(),
        blocked_recv: None,
    }));
    let sender = Sender { shared: Rc::downgrade(&shared) };
    let receiver = Receiver { shared: shared };
    (sender, receiver)
}

struct Shared<T> {
    buffer: VecDeque<T>,
    capacity: Option<usize>,
    blocked_senders: VecDeque<Task>,
    blocked_recv: Option<Task>,
}

/// The transmission end of a channel.
///
/// This is created by the `channel` function.
pub struct Sender<T> {
    shared: Weak<RefCell<Shared<T>>>,
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if let Some(task) = self.shared.upgrade().and_then(|shared| shared.borrow_mut().blocked_recv.take()) {
            // Wake up receiver to avoid deadlock in case this was the last sender
            task.unpark();
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self { Sender { shared: self.shared.clone() } }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        if let Some(shared) = self.shared.upgrade() {
            let mut shared = shared.borrow_mut();

            match shared.capacity {
                Some(capacity) if shared.buffer.len() == capacity => {
                    shared.blocked_senders.push_back(task::park());
                    Ok(AsyncSink::NotReady(msg))
                }
                _ => {
                    shared.buffer.push_back(msg);
                    if let Some(task) = shared.blocked_recv.take() {
                        task.unpark();
                    }
                    Ok(AsyncSink::Ready)
                }
            }
        } else {
            Err(SendError(msg))
        }
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }
}


/// The receiving end of a channel which implements the `Stream` trait.
///
/// This is created by the `channel` function.
pub struct Receiver<T> {
    shared: Rc<RefCell<Shared<T>>>,
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut shared = self.shared.borrow_mut();
        for task in shared.blocked_senders.drain(..) {
            task.unpark();
        }
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();            // TODO: Replace with uninhabited type, as this can never fail

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(shared) = Rc::get_mut(&mut self.shared) {
            return Ok(Async::Ready(shared.borrow_mut().buffer.pop_front()));
        }

        let mut shared = self.shared.borrow_mut();
        if let Some(msg) = shared.buffer.pop_front() {
            if let Some(task) = shared.blocked_senders.pop_front() {
                task.unpark();
            }
            Ok(Async::Ready(Some(msg)))
        } else {
            shared.blocked_recv = Some(task::park());
            Ok(Async::NotReady)
        }
    }
}

/// The transmission end of an unbounded channel.
///
/// This is created by the `unbounded` function.
pub struct UnboundedSender<T>(Sender<T>);

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self { UnboundedSender(self.0.clone()) }
}

impl<T> Sink for UnboundedSender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> { self.0.start_send(msg) }
    fn poll_complete(&mut self) -> Poll<(), SendError<T>> { Ok(Async::Ready(())) }
}

impl<T> UnboundedSender<T> {
    /// Sends the provided message along this channel.
    ///
    /// This is an unbounded sender, so this function differs from `Sink::send`
    /// by ensuring the return type reflects that the channel is always ready to
    /// receive messages.
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        let shared = match self.0.shared.upgrade() {
            Some(shared) => shared,
            None => return Err(SendError(msg)),
        };
        let mut shared = shared.borrow_mut();
        shared.buffer.push_back(msg);
        if let Some(task) = shared.blocked_recv.take() {
            task.unpark();
        }
        Ok(())
    }
}

/// The receiving end of an unbounded channel.
///
/// This is created by the `unbounded` function.
pub struct UnboundedReceiver<T>(Receiver<T>);

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;
    type Error = ();            // TODO: Replace with uninhabited type, as this can never fail

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> { self.0.poll() }
}

/// Creates an unbounded in-memory channel with buffered storage.
///
/// Identical semantics to `channel`, except with no limit to buffer size.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (send, recv) = channel_(None);
    (UnboundedSender(send), UnboundedReceiver(recv))
}

/// Error type for sending, used when the receiving end of a channel is
/// dropped
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
