use super::{Queue, MAX_CAPACITY, MAX_BUFFER};
use super::{SenderTask, ReceiverTask};
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;

#[derive(Debug)]
pub(super) struct Inner<T> {
    // Max buffer size of the channel. If `None` then the channel is unbounded.
    pub(super) buffer: Option<usize>,

    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    pub(super) state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    pub(super) message_queue: Queue<Option<T>>,

    // Atomic, FIFO queue used to send parked task handles to the receiver.
    pub(super) parked_queue: Queue<Arc<Mutex<SenderTask>>>,

    // Number of senders in existence
    pub(super) num_senders: AtomicUsize,

    // Handle to the receiver's task.
    pub(super) recv_task: Mutex<ReceiverTask>,
}

impl<T> Inner<T> {
    // The return value is such that the total number of messages that can be
    // enqueued into the channel will never exceed MAX_CAPACITY
    pub(super) fn max_senders(&self) -> usize {
        match self.buffer {
            Some(buffer) => MAX_CAPACITY - buffer,
            None => MAX_BUFFER,
        }
    }
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}
