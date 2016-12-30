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
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::ops::Deref;

use {Future, Poll, Async};
use task::{self, Task};
use lock::Lock;

/// A future that is cloneable and can be polled in multiple threads.
/// Use Future::shared() method to convert any future into a `Shared` future.
#[must_use = "futures do nothing unless polled"]
pub struct Shared<F>
    where F: Future
{
    inner: Arc<Inner<F>>,
}

struct Inner<F>
    where F: Future
{
    /// The original future.
    original_future: Lock<Option<F>>,
    /// Indicates whether the result is ready, and the state is `State::Done`.
    result_ready: AtomicBool,
    /// The state of the shared future.
    state: RwLock<State<F::Item, F::Error>>,
}

/// The state of the shared future. It can be one of the following:
/// 1. Done - contains the result of the original future.
/// 2. Waiting - contains the waiting tasks.
enum State<T, E> {
    Waiting(Vec<Task>),
    Done(Result<Arc<T>, Arc<E>>),
}

impl<F> Shared<F>
    where F: Future
{
    /// Creates a new `Shared` from another future.
    pub fn new(future: F) -> Self {
        Shared {
            inner: Arc::new(Inner {
                original_future: Lock::new(Some(future)),
                result_ready: AtomicBool::new(false),
                state: RwLock::new(State::Waiting(vec![])),
            }),
        }
    }

    fn park(&self) -> Poll<SharedItem<F::Item>, SharedError<F::Error>> {
        let me = task::park();
        match *self.inner.state.write().unwrap() {
            State::Waiting(ref mut list) => {
                list.push(me);
                Ok(Async::NotReady)
            }
            State::Done(Ok(ref e)) => Ok(SharedItem { item: e.clone() }.into()),
            State::Done(Err(ref e)) => Err(SharedError { error: e.clone() }),
        }
    }
}

impl<F> Future for Shared<F>
    where F: Future
{
    type Item = SharedItem<F::Item>;
    type Error = SharedError<F::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // The logic is as follows:
        // 1. Check if the result is ready (with result_ready)
        //  - If the result is ready, return it.
        //  - Otherwise:
        // 2. Try lock the self.inner.original_future:
        //    - If successfully locked, check again if the result is ready.
        //      If it's ready, just return it.
        //      Otherwise, poll the original future.
        //      If the future is ready, unpark the waiting tasks from
        //      self.inner.state and return the result.
        //    - If the future is not ready, or if the lock failed:
        // 3. Lock the state for write.
        // 4. If the state is `State::Done`, return the result. Otherwise:
        // 5. Create a task, push it to the waiters vector, and return
        //    `Ok(Async::NotReady)`.

        if !self.inner.result_ready.load(SeqCst) {
            match self.inner.original_future.try_lock() {
                // We already saw the result wasn't ready, but after we've
                // acquired the lock another thread could already have finished,
                // so we check `result_ready` again.
                Some(_) if self.inner.result_ready.load(SeqCst) => {}

                // If we lock the future, then try to push it towards
                // completion.
                Some(mut future) => {
                    let result = match future.as_mut().unwrap().poll() {
                        Ok(Async::NotReady) => {
                            drop(future);
                            return self.park()
                        }
                        Ok(Async::Ready(item)) => Ok(Arc::new(item)),
                        Err(error) => Err(Arc::new(error)),
                    };

                    // Free up resources associated with this future
                    *future = None;

                    // Wake up everyone waiting on the future and store the
                    // result at the same time, flagging future pollers that
                    // we're done.
                    let waiters = {
                        let mut state = self.inner.state.write().unwrap();
                        self.inner.result_ready.store(true, SeqCst);

                        match mem::replace(&mut *state, State::Done(result)) {
                            State::Waiting(waiters) => waiters,
                            State::Done(_) => {
                                panic!("store_result() was called twice")
                            }
                        }
                    };
                    for task in waiters {
                        task.unpark();
                    }
                }

                // Looks like someone else is making progress on the future,
                // let's just wait for them.
                None => return self.park(),
            }
        }

        // If we're here then we should have finished the future, so assert the
        // `Done` state and return the item/error.
        let result = match *self.inner.state.read().unwrap() {
            State::Done(ref result) => result.clone(),
            State::Waiting(_) => panic!("still waiting, not done yet"),
        };
        match result {
            Ok(e) => Ok(SharedItem { item: e }.into()),
            Err(e) => Err(SharedError { error: e }),
        }
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
        if self.inner.result_ready.load(SeqCst) {
            return
        }
        let waiters = match *self.inner.state.write().unwrap() {
            State::Waiting(ref mut waiters) => mem::replace(waiters, Vec::new()),
            State::Done(_) => return,
        };
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
