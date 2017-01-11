//! Definition of the Shared combinator, a future that is cloneable,
//! and can be polled in multiple threads.
//!
//! # Examples
//!
//! ```
//! use futures::future::*;
//!
//! let future = ok::<_, bool>(6);
//! let shared1 = future.shared();
//! let shared2 = shared1.clone();
//! assert_eq!(6, *shared1.wait().unwrap());
//! assert_eq!(6, *shared2.wait().unwrap());
//! ```

use std::mem;
use std::vec::Vec;
use std::sync::{Arc, Mutex};
use std::ops::Deref;

use {Future, Poll, Async};
use task::{self, Task};

/// A future that is cloneable and can be polled in multiple threads.
/// Use Future::shared() method to convert any future into a `Shared` future.
#[must_use = "futures do nothing unless polled"]
pub struct Shared<F: Future> {
    inner: Arc<Mutex<State<F>>>,
}

enum State<F: Future> {
    Waiting(F, Vec<Task>),
    Done(Result<Arc<F::Item>, Arc<F::Error>>),
}

impl<F> Shared<F>
    where F: Future
{
    /// Creates a new `Shared` from another future.
    pub fn new(future: F) -> Self {
        Shared {
            inner: Arc::new(Mutex::new(State::Waiting(future, Vec::new()))),
        }
    }
}

impl<F> Future for Shared<F>
    where F: Future
{
    type Item = SharedItem<F::Item>;
    type Error = SharedError<F::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        let result = match *inner {
            State::Waiting(ref mut future, _) => Some(future.poll()),
            State::Done(_) => None,
        };
        let new_state = match result {
            Some(Ok(Async::NotReady)) => None,
            Some(Ok(Async::Ready(e))) => Some(State::Done(Ok(Arc::new(e)))),
            Some(Err(e)) => Some(State::Done(Err(Arc::new(e)))),
            None => None,
        };
        let tasks_to_wake = match new_state {
            Some(new) => {
                match mem::replace(&mut *inner, new) {
                    State::Waiting(_, tasks) => tasks,
                    State::Done(_) => panic!(),
                }
            }
            None => Vec::new(),
        };

        let ret = match *inner {
            State::Waiting(_, ref mut tasks) => {
                tasks.push(task::park());
                Ok(Async::NotReady)
            }
            State::Done(Ok(ref e)) => Ok(SharedItem { item: e.clone() }.into()),
            State::Done(Err(ref e)) => Err(SharedError { error: e.clone() }.into()),
        };
        drop(inner);
        for task in tasks_to_wake {
            task.unpark();
        }
        return ret
    }
}

impl<F> Clone for Shared<F>
    where F: Future
{
    fn clone(&self) -> Self {
        Shared { inner: self.inner.clone() }
    }
}

impl<F: Future> Drop for Shared<F> {
    fn drop(&mut self) {
        // A `Shared` represents a bunch of handles to one original future
        // running on perhaps a bunch of different tasks.  That one future,
        // however, is only guaranteed to have at most one task blocked on it.
        //
        // If our `Shared` handle is the one which has the task blocked on the
        // original future, then us being dropped means that we won't ever be
        // around to wake it up again, but all the other `Shared` handles may
        // still be interested in the value of the original future!
        //
        // To handle this case we implement a destructor which will unpark all
        // other waiting tasks whenever we're dropped. This should go through
        // and wake up any interested handles, and at least one of them should
        // make its way to blocking on the original future itself.
        //
        // Note, though, that we don't call `lock` here but rather we just call
        // `try_lock`. This is done because during a `poll` above, when the lock
        // is held, we may end up calling this drop function. If that happens
        // then this `try_lock` will fail, or the `try_lock` will fail due to
        // another thread holding the lock. In both cases we're guaranteed that
        // some other thread/task other than us is blocked on the main future,
        // so there's no work for us to do.
        let mut inner = match self.inner.try_lock() {
            Ok(inner) => inner,
            Err(_) => return,
        };
        let waiters = match *inner {
            State::Waiting(_, ref mut waiters) => mem::replace(waiters, Vec::new()),
            State::Done(_) => return,
        };
        drop(inner);
        for waiter in waiters {
            waiter.unpark();
        }
    }
}

/// A wrapped item of the original future that is clonable and implements Deref
/// for ease of use.
#[derive(Debug)]
pub struct SharedItem<T> {
    item: Arc<T>,
}

impl<T> Deref for SharedItem<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.item.as_ref()
    }
}

/// A wrapped error of the original future that is clonable and implements Deref
/// for ease of use.
#[derive(Debug)]
pub struct SharedError<E> {
    error: Arc<E>,
}

impl<E> Deref for SharedError<E> {
    type Target = E;

    fn deref(&self) -> &E {
        &self.error.as_ref()
    }
}
