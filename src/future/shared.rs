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
use std::sync::{Arc, Mutex, TryLockError};
use std::ops::Deref;
use std::collections::HashMap;

use {Future, Poll, Async};
use task;

/// A future that is cloneable and can be polled in multiple threads.
/// Use Future::shared() method to convert any future into a `Shared` future.
#[must_use = "futures do nothing unless polled"]
pub struct Shared<F: Future> {
    id: u64,
    inner: Arc<Inner<F>>,
}

struct Inner<F: Future> {
    next_clone_id: Mutex<u64>,

    /// Only ever `Some` when `state` is `Waiting`. This is not part of the `State`
    /// enum because we want to be able to call `poll()` on the original future
    /// without holding a lock on `state`.
    original_future: Mutex<Option<F>>,

    state: Mutex<State<F>>,
}

enum State<F: Future> {
    Waiting(Arc<Unparker>),
    Done(Result<Arc<F::Item>, Arc<F::Error>>),
}

impl<F> Shared<F>
    where F: Future
{
    /// Creates a new `Shared` from another future.
    pub fn new(future: F) -> Self {
        Shared {
            id: 0,
            inner: Arc::new(
                Inner {
                    next_clone_id: Mutex::new(1),
                    original_future: Mutex::new(Some(future)),
                    state: Mutex::new(State::Waiting(Arc::new(Unparker::new()))),
                }),
        }
    }
}

impl<F> Future for Shared<F>
    where F: Future
{
    type Item = SharedItem<F::Item>;
    type Error = SharedError<F::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let state = self.inner.state.lock().unwrap();
        let (mut original_future, event) = match *state {
            State::Waiting(ref unparker) => {
                match self.inner.original_future.try_lock() {
                    Ok(original_future) => {
                        let mut unparker_inner = unparker.inner.lock().unwrap();
                        if unparker_inner.original_future_needs_poll {
                            unparker_inner.original_future_needs_poll = false;
                            (original_future, task::UnparkEvent::new(unparker.clone(), 0))
                        } else {
                            unparker_inner.insert(self.id, task::park());
                            return Ok(Async::NotReady)
                        }
                    }
                    Err(TryLockError::WouldBlock) => {
                        // A clone of this `Shared`, possibly on the current thread, holds the mutex.
                        // The mutex will become unlocked again after that clone finishes calling
                        // original_future.poll().
                        let mut unparker_inner = unparker.inner.lock().unwrap();
                        if unparker_inner.original_future_needs_poll {
                            // We need to try again to grab the lock on the original future. We
                            // yield and then try again, in hopes that the original_future.poll()
                            // call finishes quickly.
                            //
                            // TODO(perf): Is there a better way to deal with the case where the poll()
                            // does not finish quickly? Does the kind of situation where that might
                            // matter actually arise much in practice?
                            drop(unparker_inner);
                            task::park().unpark();
                        } else {
                            unparker_inner.insert(self.id, task::park());
                        }
                        return Ok(Async::NotReady)
                    }
                    Err(TryLockError::Poisoned(e)) => {
                        panic!("poisoned mutex: {:?}", e)
                    }
                }
            }
            State::Done(ref r) => {
                match *r {
                    Ok(ref v) => return Ok(SharedItem { item: v.clone() }.into()),
                    Err(ref e) => return Err(SharedError { error: e.clone() }.into()),
                }
            }
        };
        drop(state);

        let done_val = match *original_future {
            Some(ref mut future) => {
                match task::with_unpark_event(event, || future.poll()) {
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Ok(Async::Ready(v)) => Ok(Arc::new(v)),
                    Err(e) => Err(Arc::new(e)),
                }
            }
            None => unreachable!(),
        };

        // Lock `state` before dropping `original_future` so that another thread
        // cannot observe a situation where `original_future` is None and `state`
        // is `Waiting`.
        let mut state = self.inner.state.lock().unwrap();

        // We can now drop the original future to free up any resources it holds.
        original_future.take();
        drop(original_future);

        match mem::replace(&mut *state, State::Done(done_val.clone())) {
            State::Waiting(ref unparker) => unparker.unpark(),
            _ => unreachable!(),
        }
        drop(state);

        match done_val {
            Ok(v) => Ok(SharedItem { item: v }.into()),
            Err(e) => Err(SharedError { error: e }.into()),
        }
    }
}

impl<F> Clone for Shared<F>
    where F: Future
{
    fn clone(&self) -> Self {
        let mut next_clone_id = self.inner.next_clone_id.lock().unwrap();
        let clone_id = *next_clone_id;
        *next_clone_id += 1;
        Shared {
            id: clone_id,
            inner: self.inner.clone(),
        }
    }
}

impl<F: Future> Drop for Shared<F> {
    fn drop(&mut self) {
        if let Ok(state) = self.inner.state.lock() {
            match *state {
                State::Waiting(ref unparker) => {
                    unparker.remove(self.id);
                }
                State::Done(_) => (),
            }
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

/// An `EventSet` implementation for passing to `with_unpark_event()` when a `Shared`
/// polls its underlying future. Usually, the purpose of an `EventSet` implementation
/// is to gather precise information about what triggered an unpark, but that is *not*
/// what this implementation does. Instead, it uses `EventSet::insert()` as a hook
/// to unpark a set of waiting tasks.
struct Unparker {
    inner: Mutex<UnparkerInner>,
}

struct UnparkerInner {
    original_future_needs_poll: bool,

    /// Tasks that need to be unparked once the original future resolves.
    tasks: HashMap<u64, task::Task>,
}

impl UnparkerInner {
    fn insert(&mut self, idx: u64, task: task::Task) {
        self.tasks.insert(idx, task);
    }
}

impl task::EventSet for Unparker {
    fn insert(&self, _id: usize) {
        // The original future is ready to get polled again.
        self.unpark();
    }
}

impl Unparker {
    fn new() -> Unparker {
        Unparker {
            inner: Mutex::new(UnparkerInner{
                original_future_needs_poll: true,
                tasks: HashMap::new(),
            }),
        }
    }

    fn remove(&self, idx: u64) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.tasks.remove(&idx);
        }
    }

    fn unpark(&self) {
        let UnparkerInner { tasks, .. } = mem::replace(
            &mut *self.inner.lock().unwrap(),
            UnparkerInner {
                original_future_needs_poll: true,
                tasks: HashMap::new(),
            });

        for (_, task) in tasks {
            task.unpark();
        }
    }
}
