use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, Poll};
use slot::{Slot, Token};
use lock::Lock;
use task::{self, TaskHandle};

/// A future representing the completion of a computation happening elsewhere in
/// memory.
///
/// This is created by the `oneshot` function.
pub struct Oneshot<T> {
    inner: Arc<Inner<T>>,
    cancel_token: Option<Token>,
}

/// Represents the completion half of a oneshot through which the result of a
/// computation is signaled.
///
/// This is created by the `oneshot` function.
pub struct Complete<T> {
    inner: Arc<Inner<T>>,
    completed: bool,
}

struct Inner<T> {
    slot: Slot<Option<T>>,
    oneshot_gone: AtomicBool,
    notify_cancel: Lock<Option<TaskHandle>>,
}

/// Creates a new in-memory oneshot used to represent completing a computation.
///
/// A oneshot in this library is a concrete implementation of the `Future` trait
/// used to complete a computation from one location with a future representing
/// what to do in another.
///
/// This function is similar to Rust's channels found in the standard library.
/// Two halves are returned, the first of which is a `Oneshot` which implements
/// the `Future` trait. The second half is a `Complete` handle which is used to
/// signal the end of a computation.
///
/// Each half can be separately owned and sent across threads.
///
/// # Examples
///
/// ```
/// use futures::*;
///
/// let (c, p) = oneshot::<i32>();
///
/// p.map(|i| {
///     println!("got: {}", i);
/// }).forget();
///
/// c.complete(3);
/// ```
pub fn oneshot<T>() -> (Complete<T>, Oneshot<T>) {
    let inner = Arc::new(Inner {
        slot: Slot::new(None),
        oneshot_gone: AtomicBool::new(false),
        notify_cancel: Lock::new(None),
    });
    let oneshot = Oneshot {
        inner: inner.clone(),
        cancel_token: None,
    };
    let complete = Complete {
        inner: inner,
        completed: false,
    };
    (complete, oneshot)
}

impl<T> Complete<T> {
    /// Completes this oneshot with a successful result.
    ///
    /// This function will consume `self` and indicate to the other end, the
    /// `Oneshot`, that the error provided is the result of the computation this
    /// represents.
    pub fn complete(mut self, t: T) {
        self.completed = true;
        self.send(Some(t))
    }

    /// Polls this `Complete` half to detect whether the `Oneshot` this has
    /// paired with has gone away.
    ///
    /// This function can be used to learn about when the `Oneshot` (consumer)
    /// half has gone away and nothing will be able to receive a message sent
    /// from `complete`.
    ///
    /// Like `Future::poll`, this function will panic if it's not called from
    /// within the context of a task. In otherwords, this should only ever be
    /// called from inside another future.
    ///
    /// If `Poll::Ok` is returned then it means that the `Oneshot` has
    /// disappeared and the result this `Complete` would otherwise produce
    /// should no longer be produced.
    ///
    /// If `Poll::NotReady` is returned then the `Oneshot` is still alive and
    /// may be able to receive a message if sent. The current task, however,
    /// is scheduled to receive a notification if the corresponding `Oneshot`
    /// goes away.
    pub fn poll_cancel(&mut self) -> Poll<(), ()> {
        // Fast path up first, just read the flag and see if our other half is
        // gone.
        if self.inner.oneshot_gone.load(Ordering::SeqCst) {
            return Poll::Ok(())
        }

        // If our other half is not gone then we need to park our current task
        // and move it into the `notify_cancel` slot to get notified when it's
        // actually gone.
        //
        // If `try_lock` fails, then the `Oneshot` is in the process of using
        // it, so we can deduce that it's now in the process of going away and
        // hence we're canceled. If it succeeds then we just store our handle.
        //
        // Crucially we then check `oneshot_gone` *again* before we return.
        // While we were storing our handle inside `notify_cancel` the `Oneshot`
        // may have been dropped. The first thing it does is set the flag, and
        // if it fails to acquire the lock it assumes that we'll see the flag
        // later on. So... we then try to see the flag later on!
        let handle = task::park();
        match self.inner.notify_cancel.try_lock() {
            Some(mut p) => *p = Some(handle),
            None => return Poll::Ok(()),
        }
        if self.inner.oneshot_gone.load(Ordering::SeqCst) {
            Poll::Ok(())
        } else {
            Poll::NotReady
        }
    }

    fn send(&mut self, t: Option<T>) {
        if let Err(e) = self.inner.slot.try_produce(t) {
            self.inner.slot.on_empty(Some(e.into_inner()), |slot, item| {
                slot.try_produce(item.unwrap()).ok()
                    .expect("advertised as empty but wasn't");
            });
        }
    }
}

impl<T> Drop for Complete<T> {
    fn drop(&mut self) {
        if !self.completed {
            self.send(None);
        }
    }
}

/// Error returned from a `Oneshot<T>` whenever the correponding `Complete<T>`
/// is dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canceled;

impl<T> Future for Oneshot<T> {
    type Item = T;
    type Error = Canceled;

    fn poll(&mut self) -> Poll<T, Canceled> {
        if let Some(cancel_token) = self.cancel_token.take() {
            self.inner.slot.cancel(cancel_token);
        }
        match self.inner.slot.try_consume() {
            Ok(Some(e)) => Poll::Ok(e),
            Ok(None) => Poll::Err(Canceled),
            Err(_) => {
                let task = task::park();
                self.cancel_token = Some(self.inner.slot.on_full(move |_| {
                    task.unpark();
                }));
                Poll::NotReady
            }
        }
    }
}

impl<T> Drop for Oneshot<T> {
    fn drop(&mut self) {
        // First up, if we squirreled away a task to get notified once the
        // oneshot was filled in, we cancel that notification. We'll never end
        // up actually receiving data (as we're being dropped) so no need to
        // hold onto the task.
        if let Some(cancel_token) = self.cancel_token.take() {
            self.inner.slot.cancel(cancel_token)
        }

        // Next up, inform the `Complete` half that we're going away. First up
        // we flag ourselves as gone, and next we'll attempt to wake up any
        // handle that was stored.
        //
        // If we fail to acquire the lock on the handle, that means that a
        // `Complete` is in the process of storing one, and it'll check
        // `oneshot_gone` on its way out to see our write here.
        self.inner.oneshot_gone.store(true, Ordering::SeqCst);
        if let Some(mut handle) = self.inner.notify_cancel.try_lock() {
            if let Some(task) = handle.take() {
                drop(handle);
                task.unpark()
            }
        }
    }
}
