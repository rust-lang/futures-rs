use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, Task, Poll};
use slot::{Slot, Token};

/// A future representing the completion of a computation happening elsewhere in
/// memory.
///
/// This is created by the `promise` function.
pub struct Promise<T>
    where T: Send + 'static,
{
    inner: Arc<Inner<T>>,
    cancel_token: Option<Token>,
}

/// Represents the completion half of a promise through which the result of a
/// computation is signaled.
///
/// This is created by the `promise` function.
pub struct Complete<T>
    where T: Send + 'static,
{
    inner: Arc<Inner<T>>,
    completed: bool,
}

struct Inner<T> {
    slot: Slot<Option<T>>,
    pending_wake: AtomicBool,
}

/// Creates a new in-memory promise used to represent completing a computation.
///
/// A promise in this library is a concrete implementation of the `Future` trait
/// used to complete a computation from one location with a future representing
/// what to do in another.
///
/// This function is similar to Rust's channels found in the standard library.
/// Two halves are returned, the first of which is a `Promise` which implements
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
/// let (c, p) = promise::<i32>();
///
/// p.map(|i| {
///     println!("got: {}", i);
/// }).forget();
///
/// c.complete(3);
/// ```
pub fn promise<T>() -> (Complete<T>, Promise<T>)
    where T: Send + 'static,
{
    let inner = Arc::new(Inner {
        slot: Slot::new(None),
        pending_wake: AtomicBool::new(false),
    });
    let promise = Promise {
        inner: inner.clone(),
        cancel_token: None,
    };
    let complete = Complete {
        inner: inner,
        completed: false,
    };
    (complete, promise)
}

impl<T> Complete<T>
    where T: Send + 'static,
{
    /// Completes this promise with a successful result.
    ///
    /// This function will consume `self` and indicate to the other end, the
    /// `Promise`, that the error provided is the result of the computation this
    /// represents.
    pub fn complete(mut self, t: T) {
        self.completed = true;
        self.send(Some(t))
    }

    fn send(&mut self, t: Option<T>) {
        if let Err(e) = self.inner.slot.try_produce(t) {
            self.inner.slot.on_empty(|slot| {
                slot.try_produce(e.into_inner()).ok()
                    .expect("advertised as empty but wasn't");
            });
        }
    }
}

impl<T> Drop for Complete<T>
    where T: Send + 'static,
{
    fn drop(&mut self) {
        if !self.completed {
            self.send(None);
        }
    }
}

/// Error returned from a `Promise<T>` whenever the correponding `Complete<T>`
/// is dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Canceled;

impl<T: Send + 'static> Future for Promise<T> {
    type Item = T;
    type Error = Canceled;

    fn poll(&mut self, _: &mut Task) -> Poll<T, Canceled> {
        if self.inner.pending_wake.load(Ordering::SeqCst) {
            return Poll::NotReady
        }
        match self.inner.slot.try_consume() {
            Ok(Some(e)) => Poll::Ok(e),
            Ok(None) => Poll::Err(Canceled),
            Err(_) => Poll::NotReady,
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        if self.inner.pending_wake.load(Ordering::SeqCst) {
            if let Some(cancel_token) = self.cancel_token.take() {
                self.inner.slot.cancel(cancel_token);
            }
        }
        self.inner.pending_wake.store(true, Ordering::SeqCst);
        let inner = self.inner.clone();
        let handle = task.handle().clone();
        self.cancel_token = Some(self.inner.slot.on_full(move |_| {
            inner.pending_wake.store(false, Ordering::SeqCst);
            handle.notify();
        }));
    }
}

impl<T> Drop for Promise<T>
    where T: Send + 'static,
{
    fn drop(&mut self) {
        if let Some(cancel_token) = self.cancel_token.take() {
            self.inner.slot.cancel(cancel_token)
        }
    }
}
