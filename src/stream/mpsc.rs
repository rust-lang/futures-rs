use std::any::Any;
use std::error::Error;
use std::fmt;
use crossbeam::sync::{AtomicOption, MsQueue};
use std::sync::Arc;
use {Async, Future, Poll};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use stream::Stream;
use task::{self, Task};

const EMPTY: usize = 0;
const DATA: usize = 1;
const GONE: usize = 2;
const UNPARKED: usize = 3;
const PARKED: usize = 4;

/// `Sender` of the channel. `Sender` can be cloned.
pub struct Sender<T, E> {
    /// `Inner`
    inner: Arc<Inner<T, E>>,
    /// Count of senders, needed for determining when the last sender is GONE.
    sender_count: Arc<AtomicUsize>,
}

impl<T, E> Drop for Sender<T, E> {
    fn drop(&mut self) {
        // Each cloned sender calls `Drop` so we need to ensure that only the last sender being
        // dropped actually sets the `GONE` state.
        // The result is discarded because it could be updated in-between this line and the next.
        let _ = self.sender_count.fetch_sub(1, SeqCst);
        // if it was one before, it *should* be zero now, but let's check just to make sure
        if self.sender_count.compare_exchange(EMPTY, EMPTY, SeqCst, SeqCst).is_ok() {
            self.inner.is_sender_gone.store(GONE, SeqCst);
            // wake up receiver to go to disc event if needed
            if let Some(task) = self.inner.rx_task.take(SeqCst) {
                // NOTE: we don't need to clone the rx_task here because the last Sender is dropping
                // and therefore no `FutureSender` instances can running.
                task.unpark();
            }
        }
    }
}

impl<T, E> Sender<T, E> {
    /// Sends a value to the receiver.
    ///
    /// This method consumes the sender and returns a future which will resolve to the sender again
    /// when the value sent has been consumed.
    pub fn send(self, t: Result<T, E>) -> FutureSender<T, E> {
        FutureSender {
            fs_state: Arc::new(AtomicUsize::new(UNPARKED)),
            state: Some(FutureSenderState {
                sender: self,
                data: t,
            }),
        }
    }
}

impl<T, E> Clone for Sender<T, E> {
    fn clone(&self) -> Self {
        let _ = self.sender_count.fetch_add(1, SeqCst);
        Sender {
            inner: self.inner.clone(),
            sender_count: self.sender_count.clone(),
        }
    }
}

/// Shared state across the `Sender`(s) and `Receiver`.
struct Inner<T, E> {
    /// Stores the `Receiver` task. Only one receiver task is used and subsequently cloned
    /// by the sender when needed
    rx_task: AtomicOption<Task>,
    /// Stores `FutureSender` `Task`s along with the `FutureSender`'s `fs_state`.
    tx_tasks: MsQueue<(Task, Arc<AtomicUsize>)>,
    /// Stores the `Receiver` state, with three possible values:
    ///
    /// EMPTY - `Receiver` is not processing data
    /// DATA - `Receiver` is processing data
    /// GONE - `Receiver` is going to be/has been dropped
    rx_state: AtomicUsize,
    /// Determines whether the last sender is gone
    is_sender_gone: AtomicUsize,
    /// Stores the `Sender`'s state. This is used to ensure the `Receiver`'s stream doesn't
    /// end when there is an outbound send from the Sender (which is now GONE) that has yet to be
    /// consumed by the `Receiver`.
    ///
    /// `tx_state` has two possible values:
    ///
    /// EMPTY - no message to pending to be processed by the `Receiver`
    /// DATA - data is waiting to be processed by the `Receiver`
    tx_state: AtomicUsize,
    /// data to be sent to the `Receiver`
    data: AtomicOption<Result<T, E>>,
}

/// Receiver for the channel.
#[must_use = "streams do nothing unless polled"]
pub struct Receiver<T, E> {
    /// `Inner`
    inner: Arc<Inner<T, E>>,
    /// Tracks if `receiver.poll()` is being called for the first time on the `Receiver`. If it
    /// is the `Receiver` will make a task and set `is_first_poll_call` to false.
    is_first_poll_call: bool,
}

impl<T, E> Drop for Receiver<T, E> {
    fn drop(&mut self) {
        self.inner.rx_state.store(GONE, SeqCst);
        // drop any tx tasks, they won't be received
        if !self.inner.tx_tasks.is_empty() {
            while let Some(_) = self.inner.tx_tasks.try_pop() {}
        }

        // take data out (if there was any) since it won't be received
        let _ = self.inner.data.take(SeqCst);
        // the receiver is being dropped so the rx_task is not needed anymore
        let _ = self.inner.rx_task.take(SeqCst);
    }
}

struct FutureSenderState<T, E> {
    /// `Sender`
    sender: Sender<T, E>,
    /// Data to be sent to `Receiver`
    data: Result<T, E>,
}

impl<T, E> FutureSenderState<T, E> {
    /// try to send to receiver
    fn try_send(self, fs_state: &Arc<AtomicUsize>) -> Poll<Sender<T, E>, SendError<T, E>> {
        // check if the receiver is gone
        if self.sender.inner.rx_state.compare_exchange(GONE, GONE, SeqCst, SeqCst).is_ok() {
            Err(SendError(self.data))
        } else {
            // the first thing we can do is set the `fs_state` to GONE; we're commited to sending
            // to the receiver now. If this `FutureSender` instance had an associated task(s), then
            // the `fs_state` stored with its task will now also show dropped. The `Receiver` will
            // ensure that the task belonging to this `FutureSender` will be dropped and not bother
            // unparking it.
            fs_state.store(GONE, SeqCst);

            // NOTE: Order is important here. It is the opposite of the Receiver flow:
            // first we must store the data then we set the `tx_state`. If we do it the other
            // way, then it's possible that the `Receiver` can see DATA, but not find anything.
            // store the data
            let old = self.sender.inner.data.swap(self.data, SeqCst);
            // old value should be `None` because the `Receiver` must have consumed any prior data
            assert!(old.is_none());

            // with the data set, it is now safe to update the `tx_state` to DATA
            self.sender.inner.tx_state.store(DATA, SeqCst);
            // now attempt to wake up the `Receiver`
            if let Some(task) = self.sender.inner.rx_task.take(SeqCst) {
                let _ = self.sender.inner.rx_task.swap(task.clone(), SeqCst);
                task.unpark();
            }
            Ok(Async::Ready(self.sender))
        }
    }
}

/// A future returned by the `Sender::send` method which will resolve to the
/// `Sender` once it's available to send another message.
#[must_use = "futures do nothing unless polled"]
pub struct FutureSender<T, E> {
    /// `FutureSenderState` is used when the future will ultimately send data to the `Receiver`.
    state: Option<FutureSenderState<T, E>>,
    /// fs_state has three possible values:
    ///
    /// GONE - FutureSender is going to be dropped
    /// UNPARKED - FutureSender does not have a task parked in `tx_tasks`
    /// PARKED - FutureSender has a take parked in `tx_tasks`
    ///
    /// A clone of this `Arc` is stored in the tx_tasks queue along with the task itself. When the
    /// task is later popped by the `Receiver`, the `fs_state` is checked to see if that task's
    /// corresponding future is GONE. If so, we have no need to unpark that task, and
    /// continue popping tasks until one is found that belongs to a future that is still alive.
    ///
    /// Additionally, if during the FutureSender's processing, it determines that it already has a
    /// task parked, then it does not need to park an additional task. This logic limits the amount
    /// of tasks parked by a `FutureSender` instance to one. When the task is unparked, the
    /// `Receiver` will change the `fs_state` to UNPARKED, allowing the `FutureSender` to park a
    /// task again if needed.
    fs_state: Arc<AtomicUsize>,
}

impl<T, E> FutureSender<T, E> {
    /// try to park the future. If the future already has a related task that has been parked, no
    /// further task will be added to `tx_tasks`.
    fn try_park(&mut self, state: FutureSenderState<T, E>) -> Poll<Sender<T, E>, SendError<T, E>> {
        // if the receiver is gone, return an error
        if state.sender
            .inner
            .rx_state
            .compare_exchange(GONE, GONE, SeqCst, SeqCst)
            .is_ok() {
            Err(SendError(state.data))
        } else {
            match self.fs_state.compare_and_swap(UNPARKED, PARKED, SeqCst) {
                // we succeeded swapping, attempt to park because it was unparked before
                UNPARKED => {
                    let task = task::park();
                    // NOTE: Order is important here. We must store the tx task before waking up the
                    // receiver, otherwise the receiver won't be able to wake the FutureSender up
                    // when it's able to do so.
                    //
                    // store tx task
                    state.sender
                        .inner
                        .tx_tasks
                        .push((task, self.fs_state.clone()));

                    // It's possible that an already-running `receiver.poll()` returns NotReady
                    // before it sees this `FutureSender`'s' task in `tx_tasks`. As a result, it
                    // won't pop it from `tx_tasks` (since it is not there... yet). In order to
                    // avoid a deadlock, even a parking `FutureSender` should unpark the `Receiver`
                    // so that the `Receiver` can pop the tx task pushed above.
                    if let Some(rx_task) = state.sender.inner.rx_task.take(SeqCst) {
                        let _ = state.sender.inner.rx_task.swap(rx_task.clone(), SeqCst);
                        rx_task.unpark();
                    }
                }
                // already parked, no task to push
                PARKED => {}
                s => panic!("Unknown fs_state: `{}`.", s),
            }

            // restore our state so when we are polled again, we have it.
            self.state = Some(state);
            Ok(Async::NotReady)
        }
    }
}

impl<T, E> Future for FutureSender<T, E> {
    type Item = Sender<T, E>;
    type Error = SendError<T, E>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let state = ::std::mem::replace(&mut self.state, None);
        match state {
            Some(state) => {
                // if we don't have anything pending, try to send to receiver
                match state.sender.inner.rx_state.compare_and_swap(EMPTY, DATA, SeqCst) {
                    // we succeeded swapping, the `Receiver` is EMPTY so we try to send
                    EMPTY => state.try_send(&self.fs_state),
                    // if the `Receiver` has DATA then we try to park
                    DATA => self.try_park(state),
                    // if the `Receiver` is gone, then we error
                    GONE => Err(SendError(state.data)),
                    // should never happen
                    s => panic!("Unknown rx_state: `{}`.", s),
                }
            }
            // we should always have a state
            None => panic!("Expected `FutureSender.state` to be `Some`, found `None`."),
        }
    }
}


impl<T, E> Receiver<T, E> {
    /// Attempts to unpark one `FutureSender` task.
    ///
    /// It is possible that a task belongs to a `FutureSender` that has already completed and
    /// dropped because the executor has called subsequently polled it and that future completed
    /// before the `Receiver` had a chance to unpark it. In these cases, we need to drop that task
    /// because unparking it won't cause any activity on the `FutureSender` side and if the
    /// `Receiver` is not polled again, then we wind up in a deadlock. This function ensures that
    /// the task we pop belongs to a `FutureSender` instance that has not been dropped yet.
    fn try_unpark_tx_task(&mut self) {
        // loop until we are out of tx_tasks or find one to unpark
        while let Some((task, fs_state)) = self.inner.tx_tasks.try_pop() {
            match fs_state.compare_and_swap(GONE, GONE, SeqCst) {
                // if the future is dropped, then drop the task
                GONE => {}
                // otherwise unpark
                _ => {
                    // task is not GONE, so let's unpark and stop
                    fs_state.store(UNPARKED, SeqCst);
                    task.unpark();
                    break;
                }
            }
        }
    }
}

impl<T, E> Stream for Receiver<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // Only one rx_task needs to be made. This code will run only the first time `poll` is
        // called and it will set the rx_task. After that point, the logic in the `FutureSender` is
        // responsible for setting the rx_task, which it does by cloning the first take made here.
        if self.is_first_poll_call {
            let task = task::park();
            let _ = self.inner.rx_task.swap(task, SeqCst);
            self.is_first_poll_call = !self.is_first_poll_call;
        }

        match self.inner.tx_state.compare_and_swap(DATA, EMPTY, SeqCst) {
            // if we have DATA, then we have updated it to EMPTY and expect data to be in
            // `inner.data`
            DATA => {
                // we need to take the data first before we set rx_state, otherwise a `FutureSender`
                // could set the data before it has been taken by the receiver.
                let t = self.inner.data.take(SeqCst).expect("Receiver found no data.");
                // now that the data is taken, change rx_state from DATA to EMPTY. It is safe for a
                // `FutureSender` to send us data now.
                match self.inner.rx_state.compare_and_swap(DATA, EMPTY, SeqCst) {
                    DATA => {}
                    s => panic!("Unexpected rx_state after receiving data: `{}`", s),
                }

                // attempt to unpark a tx_task
                self.try_unpark_tx_task();

                match t {
                    Ok(t) => Ok(Some(t).into()),
                    Err(e) => Err(e),
                }
            }
            // we don't have data
            EMPTY => {
                // attempt to unpark a tx_task
                self.try_unpark_tx_task();

                // If sender is gone AND we have no data pending, then the stream is done.
                //
                // It's possible that the last `Sender` was dropped immediately before this check
                // is run, but before it was dropped, it sent another message that we need to
                // receive. Therefore, both conditions must hold for the stream to be done.
                if self.inner.is_sender_gone.load(SeqCst) == GONE &&
                   self.inner.tx_state.load(SeqCst) == EMPTY {
                    Ok(None.into())
                } else {
                    // we're not ready. `FutureSender` should unpark us when it has sent something/parked.
                    Ok(Async::NotReady)
                }
            }
            s => panic!("unexpected tx_state value: `{}`", s),
        }
    }
}


/// Creates an in-memory multiple producer, single consumer channel. The `Sender` can be cloned.
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
pub fn channel<T, E>() -> (Sender<T, E>, Receiver<T, E>) {
    let inner = Arc::new(Inner {
        tx_tasks: MsQueue::new(),
        rx_task: AtomicOption::new(),
        data: AtomicOption::new(),
        tx_state: AtomicUsize::new(EMPTY),
        rx_state: AtomicUsize::new(EMPTY),
        is_sender_gone: AtomicUsize::new(EMPTY),
    });

    // at initialization we have one `Sender`
    let sender_count = AtomicUsize::new(1);

    let tx = Sender {
        inner: inner.clone(),
        sender_count: Arc::new(sender_count),
    };
    let rx = Receiver {
        inner: inner,
        is_first_poll_call: true,
    };
    (tx, rx)
}

/// Error type returned by `FutureSender` when the receiving end of a `channel` is dropped
pub struct SendError<T, E>(Result<T, E>);

impl<T, E> fmt::Debug for SendError<T, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("SendError")
            .field(&"...")
            .finish()
    }
}

impl<T, E> fmt::Display for SendError<T, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "send failed because receiver is gone")
    }
}

impl<T, E> Error for SendError<T, E>
    where T: Any,
          E: Any
{
    fn description(&self) -> &str {
        "send failed because receiver is gone"
    }
}
