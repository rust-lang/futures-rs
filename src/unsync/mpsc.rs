//! A multi-producer, single-consumer, futures-aware, FIFO queue with back
//! pressure, for use communicating between tasks on the same thread.

use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::RefCell;

use task::{self, Task};
use sink::SendError;
use {Async, AsyncSink, Poll, StartSend, Sink, Stream};

/// Creates a bounded in-memory channel with pre-allocated storage.
///
/// This method creates concrete implementations of the `Stream` and `Sink`
/// traits which can be used to communicate a stream of values between tasks
/// with backpressure. The channel capacity is exactly `buffer`.
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>) {
    let shared = Rc::new(RefCell::new(Shared {
        buffer: VecDeque::with_capacity(buffer),
        receiver_live: true,
        blocked_senders: VecDeque::with_capacity(1),
        blocked_recv: None,
    }));
    let sender = Sender { shared: shared.clone() };
    (sender, Receiver { shared: Some(shared) })
}

struct Shared<T> {
    buffer: VecDeque<T>,
    receiver_live: bool,
    blocked_senders: VecDeque<Task>,
    blocked_recv: Option<Task>,
}

impl<T> Shared<T> {
    fn send(&mut self, msg: T) -> AsyncSink<T> {
        if self.buffer.len() == self.buffer.capacity() {
            self.blocked_senders.push_back(task::park());
            AsyncSink::NotReady(msg)
        } else {
            self.buffer.push_back(msg);
            if let Some(task) = self.blocked_recv.take() {
                task.unpark();
            }
            AsyncSink::Ready
        }
    }

    fn recv(&mut self) -> Async<T> {
        match self.buffer.pop_front() {
            None => {
                self.blocked_recv = Some(task::park());
                Async::NotReady
            }
            Some(msg) => {
                if let Some(task) = self.blocked_senders.pop_front() {
                    task.unpark();
                }
                Async::Ready(msg)
            }
        }
    }
}

/// The transmission end of a channel.
///
/// This is created by the `channel` function.
pub struct Sender<T> {
    shared: Rc<RefCell<Shared<T>>>,    
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> { Sender { shared: self.shared.clone() } }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, msg: T) -> StartSend<T, SendError<T>> {
        let mut shared = self.shared.borrow_mut();
        if !shared.receiver_live {
            Err(SendError(msg))
        } else {
            Ok(shared.send(msg))
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
    shared: Option<Rc<RefCell<Shared<T>>>>,
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.as_ref() {
            shared.borrow_mut().receiver_live = false;
        }
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();            // TODO: Replace with uninhabited type, as this can never fail

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Async::Ready(msg) = self.shared.as_ref().unwrap().borrow_mut().recv() {
            return Ok(Async::Ready(Some(msg)));
        }

        match Rc::try_unwrap(self.shared.take().unwrap()) {
            Ok(_) => Ok(Async::Ready(None)),
            Err(shared) => {
                self.shared = Some(shared);
                Ok(Async::NotReady)
            }
        }
    }
}
