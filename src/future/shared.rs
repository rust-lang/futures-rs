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
use std::sync::{Arc, Mutex};
use std::ops::Deref;
use std::collections::HashMap;
use std::vec::Vec;

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
    state: Mutex<State<F>>,
}

enum State<F: Future> {
    Waiting(Arc<Unparker>, F),
    Polling(Arc<Unparker>, Vec<task::Task>),
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
                    state: Mutex::new(State::Waiting(Arc::new(Unparker::new()), future)),
                 }),
        }
    }

    /// If this `Shared` has completed execution, returns its result immediately without
    // blocking. Otherwise, returns None.
    pub fn peek(&self) -> Option<Result<SharedItem<F::Item>, SharedError<F::Error>>> {
        match *self.inner.state.lock().unwrap() {
            State::Done(Ok(ref v)) => 
              Some(Ok(SharedItem { item: v.clone() }.into())),
            State::Done(Err(ref e)) => 
              Some(Err(SharedError { error: e.clone() }.into())),
            _ => None,
        }
    }
}

impl<F> Future for Shared<F>
    where F: Future
{
    type Item = SharedItem<F::Item>;
    type Error = SharedError<F::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut state = self.inner.state.lock().unwrap();
        let unparker = match *state {
            State::Waiting(ref unparker, ref _original_future) => {
                let mut unparker_inner = unparker.inner.lock().unwrap();
                if unparker_inner.original_future_needs_poll {
                    unparker_inner.original_future_needs_poll = false;
                    unparker.clone()
                } else {
                    unparker_inner.insert(self.id, task::park());
                    return Ok(Async::NotReady)
                }
            }
            State::Polling(ref unparker, ref mut waiters) => {
                // A clone of this `Shared` is currently calling `original_future.poll()`.
                let mut unparker_inner = unparker.inner.lock().unwrap();
                if unparker_inner.original_future_needs_poll {
                    // We need to poll the original future, but it's not here right now.
                    // So we store the current task to be unconditionally unparked once
                    // `state` is no longer `Polling`.
                    waiters.push(task::park());
                } else {
                    unparker_inner.insert(self.id, task::park());
                }
                return Ok(Async::NotReady)
            }
            State::Done(Ok(ref v)) => return Ok(SharedItem { item: v.clone() }.into()),
            State::Done(Err(ref e)) => return Err(SharedError { error: e.clone() }.into()),
        };

        let new_state = State::Polling(unparker, Vec::new());
        let (unparker, mut original_future) = match mem::replace(&mut *state, new_state) {
            State::Waiting(unparker, original_future) => (unparker, original_future),
            _ => unreachable!(),
        };
        drop(state);

        let event = task::UnparkEvent::new(unparker.clone(), 0);
        let new_state = match task::with_unpark_event(event, || original_future.poll()) {
            Ok(Async::NotReady) => State::Waiting(unparker, original_future),
            Ok(Async::Ready(v)) => State::Done(Ok(Arc::new(v))),
            Err(e) => State::Done(Err(Arc::new(e))),
        };

        let (call_unpark, result) = match new_state {
            State::Waiting(..) => (false, Ok(Async::NotReady)),
            State::Polling(..) => unreachable!(),
            State::Done(Ok(ref v)) => (true, Ok(SharedItem { item: v.clone() }.into())),
            State::Done(Err(ref e)) => (true, Err(SharedError { error: e.clone() }.into())),
        };

        let mut state = self.inner.state.lock().unwrap();
        match mem::replace(&mut *state, new_state) {
            State::Polling(unparker, waiters) => {
                if call_unpark { unparker.unpark() }
                for waiter in waiters {
                    waiter.unpark();
                }
            }
            _ => unreachable!(),
        }

        result
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
                State::Waiting(ref unparker, _) => {
                    unparker.remove(self.id);
                }
                State::Polling(ref unparker, _) => {
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
