use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use {Future, Wake, PollResult, PollError};
use executor::{Executor, DEFAULT};
use slot::{Slot, Token};
use util;

/// A future representing the completion of a computation happening elsewhere in
/// memory.
///
/// This is created by the `promise` function.
pub struct Promise<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
    token: Option<Token>,
    used: bool,
}

/// Represents the completion half of a promise through which the result of a
/// computation is signaled.
///
/// This is created by the `promise` function.
pub struct Complete<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    inner: Arc<Inner<T, E>>,
    completed: bool,
}

struct Inner<T, E> {
    slot: Slot<Option<Result<T, E>>>,
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
/// let (p, c) = promise::<i32, i32>();
///
/// p.map(|i| {
///     println!("got: {}", i);
/// }).forget();
///
/// c.finish(3);
/// ```
pub fn promise<T, E>() -> (Promise<T, E>, Complete<T, E>)
    where T: Send + 'static,
          E: Send + 'static,
{
    let inner = Arc::new(Inner {
        slot: Slot::new(None),
        pending_wake: AtomicBool::new(false),
    });
    let promise = Promise {
        inner: inner.clone(),
        token: None,
        used: false,
    };
    let complete = Complete {
        inner: inner,
        completed: false,
    };
    (promise, complete)
}

impl<T, E> Complete<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    /// Completes this promise with a successful result.
    ///
    /// This function will consume `self` and indicate to the other end, the
    /// `Promise`, that the error provided is the result of the computation this
    /// represents.
    pub fn finish(mut self, t: T) {
        self.completed = true;
        self.complete(Some(Ok(t)))
    }

    /// Completes this promise with a failed result.
    ///
    /// This function will consume `self` and indicate to the other end, the
    /// `Promise`, that the error provided is the result of the computation this
    /// represents.
    pub fn fail(mut self, e: E) {
        self.completed = true;
        self.complete(Some(Err(e)))
    }

    fn complete(&mut self, t: Option<Result<T, E>>) {
        if let Err(e) = self.inner.slot.try_produce(t) {
            self.inner.slot.on_empty(|slot| {
                slot.try_produce(e.into_inner()).ok()
                    .expect("advertised as empty but wasn't");
            });
        }
    }
}

impl<T, E> Drop for Complete<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        if !self.completed {
            self.complete(None);
        }
    }
}

struct Canceled;

impl<T: Send + 'static, E: Send + 'static> Future for Promise<T, E> {
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Option<PollResult<T, E>> {
        if self.inner.pending_wake.load(Ordering::SeqCst) {
            return None
        }
        let ret = match self.inner.slot.try_consume() {
            Ok(Some(Ok(e))) => Ok(e),
            Ok(Some(Err(e))) => Err(PollError::Other(e)),
            Ok(None) => Err(PollError::Panicked(Box::new(Canceled))),
            Err(_) if self.used => Err(util::reused()),
            Err(_) => return None,
        };
        self.used = true;
        Some(ret)
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        if self.used {
            return DEFAULT.execute(move || wake.wake())
        }
        if self.inner.pending_wake.load(Ordering::SeqCst) {
            if let Some(token) = self.token.take() {
                self.inner.slot.cancel(token);
            }
        }
        self.inner.pending_wake.store(true, Ordering::SeqCst);
        let inner = self.inner.clone();
        self.token = Some(self.inner.slot.on_full(move |_| {
            inner.pending_wake.store(false, Ordering::SeqCst);
            wake.wake()
        }));
    }

    fn tailcall(&mut self) -> Option<Box<Future<Item=T, Error=E>>> {
        None
    }
}

impl<T, E> Drop for Promise<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.inner.slot.cancel(token)
        }
    }
}
