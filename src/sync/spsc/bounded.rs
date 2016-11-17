use std::any::Any;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use {Poll, Async, StartSend, AsyncSink};
use lock::Lock;
use stream::Stream;
use sink::Sink;
use task::{self, Task};

/// Creates an in-memory channel implementation of the `Stream` trait.
///
/// This method creates a concrete implementation of the `Stream` trait which
/// can be used to send values across threads in a streaming fashion. This
/// channel is unique in that it implements back pressure to ensure that the
/// sender never outpaces the receiver. The `Sender::send` method will only
/// allow sending one message and the next message can only be sent once the
/// first was consumed.
///
/// The `Receiver` returned implements the `Stream` trait and has access to any
/// number of the associated combinators for transforming the result.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        state: AtomicUsize::new(EMPTY),
        data: Lock::new(None),
        rx_task1: Lock::new(None),
        rx_task2: Lock::new(None),
        tx_task1: Lock::new(None),
        tx_task2: Lock::new(None),
    });
    let sender = Sender {
        inner: inner.clone(),
        flag: true,
    };
    let receiver = Receiver {
        inner: inner,
        flag: true,
    };
    (sender, receiver)
}

/// The transmission end of a channel which is used to send values.
///
/// This is created by the `channel` method in the `stream` module.
pub struct Sender<T> {
    // Option to signify early closure of the sending side
    inner: Arc<Inner<T>>,
    // described below on `Inner`
    flag: bool,
}

/// The receiving end of a channel which implements the `Stream` trait.
///
/// This is a concrete implementation of a stream which can be used to represent
/// a stream of values being computed elsewhere. This is created by the
/// `channel` method in the `stream` module.
#[must_use = "streams do nothing unless polled"]
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
    // described below on `Inner`
    flag: bool,
}

/// Internal state shared by the `Sender` and `Receiver` types.
///
/// While this is similar to the oneshot internal state, it's subtly different
/// with an extra `rx_task` and `tx_task` fields for blocking. See comments
/// below for what's what.
struct Inner<T> {
    /// Actual state of this channel, essentially what's going on inside `data`.
    ///
    /// This currently has three valid values (constants below this struct
    /// definition):
    ///
    /// * EMPTY - both the sender and receiver are alive, but there's no data to
    ///           be had in `data`. A receiver must block and a sender can
    ///           proceed.
    /// * DATA - both the sender and receiver are alive, and there's data to be
    ///          retrieved inside of `data`. A receiver can proceed by picking
    ///          out the data but a sender must block to send something else.
    /// * GONE - *either* the sender or receiver is gone (or both). No operation
    ///          should block any more and all data should be handled
    ///          appropriately. Note that if a receiver sees GONE then there may
    ///          still be data inside `data`, so it needs to be checked.
    ///
    /// This isn't really atomically updated in the sense of swap or
    /// compare_exchange, but rather with a few atomic stores here and there
    /// with a sprinkling of compare_exchange. See the code below for more
    /// details.
    state: AtomicUsize,

    /// The actual data being transmitted across this channel.
    ///
    /// This is `Some` if state is `DATA` and `None` if state is `EMPTY`. If
    /// the state is `GONE` then the receiver needs to check this and the sender
    /// should ignore it.
    ///
    /// Note that this probably doesn't need a `Lock` around it and can likely
    /// just be an `UnsafeCell`
    data: Lock<Option<T>>,

    /// Ok, here's where things get tricky. These four fields are for blocked
    /// tasks.
    ///
    /// "Four?!" you might be saying, "surely there can only be at most one task
    /// blocked on a channel" you might also be saying. Well, you're correct!
    /// Due to various subtleties and the desire to never have any task *block*
    /// another (in the sense of a mutex block) these are all required.
    ///
    /// The general gist of what's going on here is that a `Sender` will
    /// alternate storing its blocked task in `tx_task1` and `tx_task2`.
    /// Similarly a `Receiver` will alternate storing a blocked task in
    /// `rx_task1` and `rx_task2`.
    ///
    /// The race that this is trying to solve is this:
    ///
    /// * When the receiver receives a message, it will empty out the data, then
    ///   lock the tx task to wake it up (if one's available).
    /// * The sender, not blocked, then sees that the channel is empty, so it
    ///   sends some data.
    /// * The sender again, not blocked, tries to send some more data, but this
    ///   time its blocked.
    ///
    /// Here we've got a concurrent situation where the receiver is holding the
    /// locked for the tx task, but the sender *also* wants to store a new tx
    /// task to get unblocked. This would involve the sender literally blocking
    /// waiting for the receiver to unlock, so instead we shard up the tx task
    /// locks into two. This means that in the situation above the two halves
    /// will never be racing on the same slot and always have access to block
    /// when they need it.
    ///
    /// Similar logic applies to the receiver (I think...) so there's two rx
    /// task slots here as well.
    ///
    /// TODO: does this really need two rx tasks? I've thought through tx task
    ///       to justify those two but not the other way around.
    rx_task1: Lock<Option<Task>>,
    rx_task2: Lock<Option<Task>>,
    tx_task1: Lock<Option<Task>>,
    tx_task2: Lock<Option<Task>>,
}

const EMPTY: usize = 0;
const DATA: usize = 1;
const GONE: usize = 2;

/// Error type for sending, used when the receiving end of the channel is dropped
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

impl<T> Error for SendError<T>
    where T: Any
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

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        // First thing's first, let's check out the state of the channel. A
        // local flag is kept which indicates whether we've got data available
        // to us.
        let mut data = false;
        match self.inner.state.load(SeqCst) {
            // If the sender is gone, then we need to take a look inside our
            // `data` field. Fall through below to figure that out.
            GONE => data = true,

            // If we've got data, then we've got data!
            DATA => data = true,

            // If the channel thinks it's empty, then we need to try to block.
            // Take our task and put it in the appropriate slot. If we can't
            // acquire the lock on the slot, then we know it's taken because a
            // sender put some data in the channel and it's trying to wake us
            // up. In that situation we know we've got data.
            EMPTY => {
                let task = task::park();
                match self.rx_task().try_lock() {
                    Some(mut slot) => *slot = Some(task),
                    None => data = true,
                }
            }

            n => panic!("bad state: {}", n),
        }

        // If we *didn't* get data above, then we stored our task to be woken up
        // at a later time. Recheck to cover the race where right before we
        // stored the task a sender put some data on the channel. If we still
        // see `EMPTY`, however, then we're guaranteed any future sender will
        // wake up our task.
        if !data && self.inner.state.load(SeqCst) == EMPTY {
            return Ok(Async::NotReady)
        }

        // We've gotten this far, so extract the data (which is guaranteed to
        // not be contended) and transform it to our return value.
        let ret = Ok(self.inner.data.try_lock().unwrap().take().into());

        // Inform the channel that our data slot is now empty. Note that we use
        // a compare_exchange here to ensure that if the sender goes away (e.g.
        // transitions to GONE) we don't paper over that state.
        drop(self.inner.state.compare_exchange(DATA, EMPTY, SeqCst, SeqCst));

        // Now that we've extracted the data and updated the state of the
        // channel, it's time for us to notify any blocked sender that it can
        // continue to move along if it needs. Take a peek at the tx_task we're
        // waking up and if it's there unpark it.
        //
        // TODO: Should this try_lock be an unwrap()? I... can't think of a case
        //       where the sender should be interfering with this.
        if let Some(mut slot) = self.tx_task().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }

        // And finally, with our successfuly receiving of a message, we flip our
        // flag to switch the slots we're thinking of for rx tasks and tx tasks.
        self.flag = !self.flag;
        return ret
    }
}

impl<T> Receiver<T> {
    /// Helper method to look at the right slot to store an rx task into, given
    /// how many messages we've sent so far.
    fn rx_task(&self) -> &Lock<Option<Task>> {
        if self.flag {
            &self.inner.rx_task1
        } else {
            &self.inner.rx_task2
        }
    }

    /// Helper method to look at the right slot to store an tx task into, given
    /// how many messages we've sent so far.
    fn tx_task(&self) -> &Lock<Option<Task>> {
        if self.flag {
            &self.inner.tx_task1
        } else {
            &self.inner.tx_task2
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // First up, inform our sender bretheren that we're going away by
        // transitioning ourselves to the GONE state.
        if self.inner.state.swap(GONE, SeqCst) == DATA {
            drop(self.inner.data.try_lock().unwrap().take());
        }

        // Next, if we stored a handle to our own task to get woken up then
        // we're sure to drop that here. No need to keep that around and we
        // don't want to hold onto any stale references.
        if let Some(mut slot) = self.rx_task().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                drop(task);
            }
        }

        // And finally, if any sender was waiting for us to take some data we
        // ... well ... took the data! If they're here then wake them up.
        if let Some(mut slot) = self.tx_task().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }
    }
}

impl<T> Sender<T> {
    /// Same as Receiver::rx_task above.
    fn rx_task(&self) -> &Lock<Option<Task>> {
        if self.flag {
            &self.inner.rx_task1
        } else {
            &self.inner.rx_task2
        }
    }

    /// *Almost* the same as Receiver::tx_task above were it not for the lone
    /// `!` in front of `self.flag`.
    ///
    /// Here we actually invert what we're looking at to ensure that the
    /// receiver and the sender are always trying to deal with the same task
    /// blocking location.
    ///
    /// For example if the sender has not received anything, it'll wake up
    /// blocked tasks in `tx_task1`. If we've sent something, however, our flag
    /// will be the opposite and then when we block we want the receiver to wake
    /// us up. As a result, we invert our logic to block in the same location.
    fn tx_task(&self) -> &Lock<Option<Task>> {
        if !self.flag {
            &self.inner.tx_task1
        } else {
            &self.inner.tx_task2
        }
    }
}

impl<T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, item: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError>
    {
        // This is very similar to `Receiver::poll` above, it's basically just
        // the opposite in a few places.
        let mut empty = false;
        match self.inner.state.load(SeqCst) {
            GONE => return Err(SendError(item)),
            EMPTY => empty = true,
            DATA => {
                let task = task::park();
                match self.tx_task().try_lock() {
                    Some(mut slot) => *slot = Some(task),
                    None => empty = true,
                }
            }
            n => panic!("bad state: {}", n),
        }
        if !empty {
            match self.inner.state.load(SeqCst) {
                // If there's still data on the channel we've successfully
                // blocked, so return.
                DATA => return Ok(AsyncSink::NotReady(item)),

                // If the receiver is gone, inform so immediately. The receiver
                // may also be looking at `self.inner.data` at this point so we have
                // to avoid it.
                GONE => return Err(SendError(item)),

                // Oh oops! Looks like we blocked ourselves but during that time
                // the data was taken, let's fall through and send our data.
                EMPTY => {}
                n => panic!("bad state: {}", n),
            }
        }
        *self.inner.data.try_lock().unwrap() = Some(item);
        drop(self.inner.state.compare_exchange(EMPTY, DATA, SeqCst, SeqCst));
        if let Some(mut slot) = self.rx_task().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }

        self.flag = !self.flag;
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // Like Receiver::drop we let our other half know we're gone, and then
        // we try to wake them up if they're waiting for us. Note that we don't
        // frob tx_task here as that's done in FutureSender down below.
        self.inner.state.store(GONE, SeqCst);

        if let Some(mut slot) = self.rx_task().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }

        // If we've registered a task to be interested in when the sender was
        // empty again, wake up the task. (This almost certainly means that the
        // client code is buggy, but better to generate an extra wakeup than
        // obscure the bug via deadlock.)
        if let Some(mut slot) = self.tx_task().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }
    }
}
