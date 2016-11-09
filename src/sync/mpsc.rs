//! A multi-producer, single-consumer, futures-aware channel

use {Async, Poll};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicUsize, AtomicBool};
use stream::Stream;
use std::thread;
use task::{self, Task};
use std::sync::{Arc, Mutex, mpsc};
use lock::Lock;

const PARKED: usize = 4;
const UNPARKING: usize = 5;
const UNPARKED: usize = 6;

/// `Sender` of the channel. `Sender` can be cloned.
pub struct Sender<T> {
    /// Sender
    tx: mpsc::Sender<T>,
    /// Inner
    inner: Arc<Inner>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        let _ = self.inner.num_senders.fetch_add(1, SeqCst);
        Sender {
            tx: self.tx.clone(),
            inner: self.inner.clone(),
        }
    }
}

/// Shared state across the `Sender`(s) and `Receiver`.
struct Inner {
    /// Stores a `Receiver` task.
    rx_task: Mutex<Option<Task>>,
    rx_state: AtomicUsize,
    tx_state: AtomicUsize,
    /// Stores an additional `Receiver` task that will only be unparked when the last `Sender`
    /// drops. We ensure there is always a `Receiver` task available for the last `Sender` to
    /// unpark the `Receiver`.
    rx_task_tx_drop: Lock<Option<Task>>,
    /// Number `Sender`s. Needed for determining when the last sender has been dropped.
    num_senders: AtomicUsize,
    /// Number of messages sent to the `Receiver` but not yet processed. Needed to determine whether
    /// to expect a value when the Receiver's first attempt to get data returns Empty.
    num_msgs_pending: AtomicUsize,
    /// Is set to false after `poll` is called on the `Receiver` for the first time.
    is_first_rx_poll: AtomicBool,
}

/// Receiver for the channel.
#[must_use = "streams do nothing unless polled"]
pub struct Receiver<T> {
    /// Receiver
    rx: mpsc::Receiver<T>,
    /// Inner
    inner: Arc<Inner>,
}

impl<T> Sender<T> {
    /// Send a message on the channel
    pub fn send(&mut self, t: T) -> Result<(), mpsc::SendError<T>> {
        println!("TX: poll");
        match self.tx.send(t) {
            Ok(_) => {
                let _ = self.inner.num_msgs_pending.fetch_add(1, SeqCst);
                match self.inner.tx_state.compare_and_swap(UNPARKED, UNPARKING, SeqCst) {
                    UNPARKED => {
                        match self.inner.rx_state.load(SeqCst) {
                            PARKED => {
                                println!("TX: unpark rx task");
                                if let Ok(mut task) = self.inner
                                    .rx_task
                                    .lock() {
                                    task.take().unwrap().unpark();
                                    self.inner.rx_state.store(UNPARKED, SeqCst);
                                    self.inner.tx_state.store(UNPARKED, SeqCst);
                                }
                            }
                            UNPARKED => {
                                println!("TX: RX has nothing to unpark");
                                self.inner.tx_state.store(UNPARKED, SeqCst);
                            }
                            s => panic!("TX: unknown state: {}", s),
                        }
                    }
                    _ => {}
                }
                println!("TX: poll END");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // Each cloned sender calls `Drop` so we need to ensure that only the last sender being
        // dropped will unpark the `Receiver`.
        if self.inner.num_senders.fetch_sub(1, SeqCst) == 1 &&
           !self.inner.is_first_rx_poll.load(SeqCst) {
            // wake up receiver to go to disconnection event or get more data if there is any
            // can unwrap because we are guaranteed the receiver has set this value by now
            let task = self.inner.rx_task_tx_drop.try_lock().unwrap().take().unwrap();
            task.unpark();
        }
    }
}

/// Creates a new unbounded multiple-producer, single-consumer channel. `Sender` can be cloned.
///
/// This channel does not implement back pressure.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    let inner = Arc::new(Inner {
        rx_task: Mutex::new(None),
        rx_task_tx_drop: Lock::new(None),
        tx_state: AtomicUsize::new(UNPARKED),
        rx_state: AtomicUsize::new(UNPARKED),
        num_senders: AtomicUsize::new(1),
        num_msgs_pending: AtomicUsize::new(0),
        is_first_rx_poll: AtomicBool::new(true),
    });
    let tx = Sender {
        tx: tx,
        inner: inner.clone(),
    };
    let rx = Receiver {
        rx: rx,
        inner: inner,
    };
    (tx, rx)
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("RX: poll");
        // if this is the receiver's first poll call, store two receiver tasks:
        // rx_task_tx_drop - used only once to wake up receiver when last Sender drops
        // rx_task - used by senders to wake up the receiver after a message has been sent
        if self.inner.is_first_rx_poll.load(SeqCst) {
            let mut task = self.inner.rx_task_tx_drop.try_lock().unwrap();
            *task = Some(task::park());
            self.inner.is_first_rx_poll.store(false, SeqCst);
        }

        match self.rx.try_recv() {
            Ok(t) => {
                let _ = self.inner.num_msgs_pending.fetch_sub(1, SeqCst);
                Ok(Some(t).into())
            }
            Err(mpsc::TryRecvError::Disconnected) => Ok(Async::Ready(None)),
            Err(mpsc::TryRecvError::Empty) => {
                match (self.inner.num_senders.load(SeqCst),
                       self.inner.num_msgs_pending.load(SeqCst)) {
                    // if we have no senders and no pending messages, the stream is over.
                    (0, 0) => Ok(Async::Ready(None)),
                    // we have message(s) pending
                    (_, num_msgs_pending) if num_msgs_pending != 0 => {
                        loop {
                            match self.rx.try_recv() {
                                Ok(t) => {
                                    let _ = self.inner.num_msgs_pending.fetch_sub(1, SeqCst);
                                    return Ok(Some(t).into());
                                }
                                // In the case of the last message being sent by the last `Sender`
                                // it's possible that the receiver is polled by some other means,
                                // e.g. during Drop of the sender, while we are in this loop and
                                // takes the last message. In these cases, if we get a Disconnected
                                // error, then the stream is over.
                                Err(mpsc::TryRecvError::Disconnected) => {
                                    return Ok(Async::Ready(None));
                                }
                                // if we are still Empty, then we should get a message soon
                                Err(mpsc::TryRecvError::Empty) => thread::yield_now(),
                            }
                        }
                    }
                    // otherwise we park
                    _ => {
                        if let Ok(mut task) = self.inner.rx_task.lock() {
                            *task = Some(task::park());
                            self.inner.rx_state.store(PARKED, SeqCst);
                        }
                        println!("RX: parked");
                        println!("RX: end");

                        Ok(Async::NotReady)
                    }
                }
            }
        }
    }
}
