use std::any::Any;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize};
use std::sync::atomic::Ordering::SeqCst;

use {Poll, Async, StartSend, AsyncSink};
use lock::Lock;
use stream::Stream;
use sink::Sink;
use task::{self, Task};
use sync::spsc::queue::Queue;

/// Creates an unbounded sender/receiver pair.
///
/// This function creates a new single-producer, single-consumer (spsc) channel
/// where the sender cannot be cloned and all messages are buffered internally.
/// That is, this channel provides no backpressure as it will buffer items
/// unboundedly. For a bounded and backpressure-based solution, see the
/// `channel` function.
///
/// An unbounded channel's sender is always ready to send a message and all
/// message sends succeed so long as the receiver has not gone away yet. Note
/// that successfully sending a message **does not imply receiving the message**
/// as the receiver is not guaranteed to receive all messages before being
/// dropped.
///
/// Also note that the lack of backpressure here can be a risky choice for some
/// situations and may lead to situations such as resource exhaustion if a
/// system is overloaded. Care should be taken to ensure there are limits on the
/// system elsewhere to prevent this case.
pub fn unbounded<T, E>() -> (UnboundedSender<T, E>, UnboundedReceiver<T, E>) {
    let inner = Arc::new(Inner {
        queue: unsafe { Queue::new(128) },
        closed: AtomicBool::new(false),
        active_sends: AtomicIsize::new(0),
        blocker1: Lock::new(None),
        blocker2: Lock::new(None),
    });

    let tx = UnboundedSender { inner: inner.clone(), flag: false };
    let rx = UnboundedReceiver { inner: inner.clone(), flag: false };
    (tx, rx)
}

/// The transmission half of the unbounded spsc channel.
///
/// This structure can be used to send messages to a receiver, and all messages
/// will be buffered internally.
///
/// Each sender implements the `Sink` trait but also provide an inherent `send`
/// method to statically handle the case that `NotReady` never arises.
pub struct UnboundedSender<T, E> {
    inner: Arc<Inner<Result<T, E>>>,

    /// Internal flag that's flipped on each message sent and indicates which
    /// blocker slot inside `Inner` should be awoken to receive a message.
    flag: bool,
}

/// The receiving half of the unbounded spsc channel.
///
/// This structure can be used to receive messages from a sender, blocking until
/// one is available.
///
/// Each receiver implements the `Stream` trait for items sent over the channel.
pub struct UnboundedReceiver<T, E> {
    inner: Arc<Inner<Result<T, E>>>,
    flag: bool,
}

/// Internal state of the sender/receiver pair, essentially the shared state of
/// the channel.
struct Inner<T> {
    /// Lock-free queue that messages are pushed on to. This is vendored from
    /// the standard library and provides just standard push/pop functions, but
    /// they're unsafe as we need to provide the guarantee that only one thread
    /// calls `push` and only one calls `pop`.
    queue: Queue<T>,

    /// Indication whether the receiver has been dropped. If true then all
    /// future message sends should be blocked.
    closed: AtomicBool,

    /// TODO: dox
    active_sends: AtomicIsize,

    /// Two slots for receiver tasks which can be blocked. For information on
    /// why there's two here see the comments in `bounded.rs`.
    blocker1: Lock<Option<Task>>,
    blocker2: Lock<Option<Task>>,
}

/// The type of error returned from the `send` method and the sender's `Sink`
/// implementation.
///
/// This error is returned when a message is sent but the receiver is guaranteed
/// to have gone away.
pub struct UnboundedSendError<T, E>(Result<T, E>);

impl<T, E> fmt::Debug for UnboundedSendError<T, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("UnboundedSendError")
            .field(&"...")
            .finish()
    }
}

impl<T, E> fmt::Display for UnboundedSendError<T, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "send failed because receiver is gone")
    }
}

impl<T, E> Error for UnboundedSendError<T, E>
    where T: Any, E: Any
{
    fn description(&self) -> &str {
        "send failed because receiver is gone"
    }
}

impl<T, E> UnboundedReceiver<T, E> {
    // this function is only safe too call on one thread at a time, hence the
    // `unsafe` annotation.
    unsafe fn pop(&mut self) -> Option<Result<T, E>> {
        let res = self.inner.queue.pop();
        if res.is_some() {
            self.flag = !self.flag;
        }
        return res
    }

    fn blocker(&self) -> &Lock<Option<Task>> {
        if self.flag {
            &self.inner.blocker1
        } else {
            &self.inner.blocker2
        }
    }
}

impl<T, E> Stream for UnboundedReceiver<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<T>, E> {
        let mut blocked = false;
        let mut n = 0;

        loop {
            assert!(n < 2);
            // First, try to pick off a message.
            //
            // The unsafety here should be ok as we just need to provide the
            // guarantee that `pop` isn't called concurrently on multipled
            // threads (for the inner queue). The sender only calls poll once
            // we've been dropped, and we haven't been dropped yet, so we're the
            // only one calling pop.
            match unsafe { self.pop() } {
                Some(Ok(e)) => return Ok(Some(e).into()),
                Some(Err(e)) => return Err(e),
                None => {}
            }

            // If we didn't have a message, check to see if the sender is gone
            if self.inner.closed.load(SeqCst) {
                return Ok(None.into())
            }

            // Ok, we've for sure missed everything in the queue, so if we've
            // already blocked then we're definitely not ready.
            if blocked {
                return Ok(Async::NotReady)
            }

            // Ok, we haven't previously blocked, the sender is still there, so
            // let's block waiting for the next message. If we miss the lock
            // here then we're guaranteed that the sender is locking it to tell
            // us something we missed above. Otherwise, if we store ourselves,
            // then we check again to make sure that while we were blocking a
            // sender didn't sneak in to send us a message.
            assert!(n == 0, "ran through the loop too many times");
            let task = task::park();
            if let Some(mut slot) = self.blocker().try_lock() {
                *slot = Some(task);
                blocked = true;
            }

            n += 1;
        }
    }
}

impl<T, E> Drop for UnboundedReceiver<T, E> {
    fn drop(&mut self) {
        // First we try to clear out the queue, which will implicitly tell
        // senders that we're going away, for more info see the docs on that
        // method.
        self.inner.maybe_clean_queue();

        // Remove our blocked task if one exists, no need to hold on to that.
        // Note that if we miss the locks here it's because the sender's waking
        // us up, so they're also removing the task.
        if let Some(mut slot) = self.inner.blocker1.try_lock() {
            let task = slot.take();
            drop(slot);
            drop(task);
        }
        if let Some(mut slot) = self.inner.blocker2.try_lock() {
            let task = slot.take();
            drop(slot);
            drop(task);
        }
    }
}

impl<T, E> UnboundedSender<T, E> {
    /// Sends a message over this channel to the receiver.
    ///
    /// The message `t` is guaranteed to be sent and will be buffered internally
    /// so long as the receiver is still alive. If the receiver has dropped then
    /// the message is returned as an `UnboundedSendError`. If the message was
    /// queued then `Ok(())` is returned.
    pub fn send(&mut self, t: Result<T, E>) -> Result<(), UnboundedSendError<T, E>> {
        // First up, inform the receiver that we're about to start sending a
        // message. This increment will normally bump the count to 1. If,
        // however, we bumped it to 0 (e.g. we see -1) then it means that the
        // receiver has already gone away. In that case we fixup the count and
        // return an error.
        if self.inner.active_sends.fetch_add(1, SeqCst) == -1 {
            self.inner.active_sends.fetch_sub(1, SeqCst);
            return Err(UnboundedSendError(t))
        }

        // Next, actually enqueue our message.
        //
        // Note that the unsafety here stems from the fact that only one thread
        // can call `push` on the queue safely at any one point in time. Here,
        // though, this is the only case of calling `push` and we have a mutable
        // reference to our non-cloneable sender, so we should satisfy that
        // guarantee statically.
        unsafe {
            self.inner.queue.push(t);
        }

        // Here we restore the `active_sends` back to 0. If, however, we take it
        // down to -1 then it means that while we were pushing the receiver went
        // away and now it's our job to clean up the queue. In that case, we
        // clean up the queue here.
        self.inner.maybe_clean_queue();

        // And finally, now that we've sent a message, try to unblock a blocker.
        // If we miss the lock then it's held by the receiver and when they spin
        // through the loop again they'll see our message.
        //
        // Note that we also flip `flag` here to alternate which slot we're
        // going to be looking in for a blocker next time.
        if let Some(mut slot) = self.blocker().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }
        self.flag = !self.flag;

        Ok(())
    }

    fn blocker(&self) -> &Lock<Option<Task>> {
        if self.flag {
            &self.inner.blocker1
        } else {
            &self.inner.blocker2
        }
    }
}

impl<T, E> Sink for UnboundedSender<T, E> {
    type SinkItem = Result<T, E>;
    type SinkError = UnboundedSendError<T, E>;

    fn start_send(&mut self, item: Self::SinkItem)
                  -> StartSend<Self::SinkItem, Self::SinkError>
    {
        UnboundedSender::send(self, item).map(|()| AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

impl<T, E> Drop for UnboundedSender<T, E> {
    fn drop(&mut self) {
        // Inform the receiver that we're gone, and then unblock them if we're
        // waiting. Note that if we miss the lock here it means that the
        // receiver holds it and will see our `closed` flag next time through
        // the loop.s
        self.inner.closed.store(true, SeqCst);
        if let Some(mut slot) = self.blocker().try_lock() {
            if let Some(task) = slot.take() {
                drop(slot);
                task.unpark();
            }
        }
    }
}

impl<T> Inner<T> {
    /// Attempt to clear out the queue of all remaining messages.
    ///
    /// This is called whenever a receiver is dropped and also called by senders
    /// when they finish pushing. The thinking here is to ensure that as soon as
    /// the receiver drops all buffered messages are dropped ASAP.
    fn maybe_clean_queue(&self) {
        // First thing is to decrement the number of active sends that are
        // happening. For senders this typically returns `1` because they
        // increase this count before pushing. For receivers, however, this may
        // return 0 if no sender is actively pushing.
        //
        // Regardless, precisely one thread should see `0` here, moving the
        // number of sends to -1. That thread is the last thread exiting and
        // then has ownership of the queue. At that time all buffered messages
        // are popped and dropped.
        //
        // Note that the synchronization here should provide us the safety we
        // need to call the `unsafe` function pop (which needs at most one
        // thread calling it).
        if self.active_sends.fetch_sub(1, SeqCst) != 0 {
            return
        }

        while let Some(msg) = unsafe { self.queue.pop() } {
            drop(msg);
        }
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        // Sanity check that we did indeed drop everything
        unsafe {
            assert!(self.queue.pop().is_none());
        }
    }
}
