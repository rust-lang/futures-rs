use std::sync::Arc;

use {Future, Poll};
use slot::{Slot, Token};
use task;

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
        if let Some(cancel_token) = self.cancel_token.take() {
            self.inner.slot.cancel(cancel_token)
        }
    }
}
