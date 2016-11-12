use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::ops::Deref;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::cell::UnsafeCell;
use std::marker::Sync;
use {Future, Poll, Async};
use task::{self, Task};
use lock::Lock;


/// A wrapped item of the original future.
/// It is clonable and implements Deref for ease of use.
#[derive(Debug)]
pub struct SharedItem<T> {
    item: Arc<T>,
}

impl<T> Clone for SharedItem<T> {
    fn clone(&self) -> Self {
        SharedItem { item: self.item.clone() }
    }
}

impl<T> Deref for SharedItem<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.item.as_ref()
    }
}

/// A wrapped error of the original future.
/// It is clonable and implements Deref for ease of use.
#[derive(Debug)]
pub struct SharedError<E> {
    error: Arc<E>,
}

impl<T> Clone for SharedError<T> {
    fn clone(&self) -> Self {
        SharedError { error: self.error.clone() }
    }
}

impl<E> Deref for SharedError<E> {
    type Target = E;

    fn deref(&self) -> &E {
        &self.error.as_ref()
    }
}

impl<T> SharedItem<T> {
    fn new(item: T) -> Self {
        SharedItem { item: Arc::new(item) }
    }
}

impl<E> SharedError<E> {
    fn new(error: E) -> Self {
        SharedError { error: Arc::new(error) }
    }
}

/// The data that has to be synced to implement `Shared`,
/// in order to satisfy the `Future` trait's constraints.
struct SyncedInner<F>
    where F: Future
{
    original_future: F, // The original future
}

struct Inner<F>
    where F: Future
{
    synced_inner: Lock<SyncedInner<F>>,
    tasks_unpark_started: AtomicBool,
    /// When original future is polled and ready, unparks all the tasks in that channel
    tasks_receiver: Lock<Receiver<Task>>,
    /// The original future result wrapped with `SharedItem`/`SharedError`
    result: UnsafeCell<Option<Result<Async<SharedItem<F::Item>>, SharedError<F::Error>>>>,
}

unsafe impl<F> Sync for Inner<F> where F: Future {}

/// TODO: doc
#[must_use = "futures do nothing unless polled"]
pub struct Shared<F>
    where F: Future
{
    inner: Arc<Inner<F>>,
    tasks_sender: Sender<Task>,
}

pub fn new<F>(future: F) -> Shared<F>
    where F: Future
{
    let (tasks_sender, tasks_receiver) = channel();
    Shared {
        inner: Arc::new(Inner {
            synced_inner: Lock::new(SyncedInner { original_future: future }),
            tasks_unpark_started: AtomicBool::new(false),
            tasks_receiver: Lock::new(tasks_receiver),
            result: UnsafeCell::new(None),
        }),
        tasks_sender: tasks_sender,
    }
}

impl<F> Future for Shared<F>
    where F: Future
{
    type Item = SharedItem<F::Item>;
    type Error = SharedError<F::Error>;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // The logic is as follows:
        // 1. Check if the result is ready (with tasks_unpark_started)
        //  - If the result is ready, return it.
        //  - Otherwise:
        // 2. Try lock the self.inner.synced_inner:
        //    - If successfully locked, poll the original future.
        //      If the future is ready, unpark the tasks from
        //      self.inner.tasks_receiver and return the result.
        //    - If the future is not ready:
        // 3. Create a task and send it through self.tasks_sender.
        // 4. Check again if the result is ready (with tasks_unpark_started)
        // 5. Return the result if it's ready. It is necessary because otherwise there could be
        //    a race between the task sending and the thread receiving the tasks.

        let mut should_unpark_tasks: bool = false;

        // If the result is ready, just return it
        if self.inner.tasks_unpark_started.load(Ordering::Relaxed) {
            unsafe {
                if let Some(ref result) = *self.inner.result.get() {
                    return result.clone();
                }
            }
        }

        // The result was not ready.
        match self.inner.synced_inner.try_lock() {
            Some(mut inner_guard) => {
                let ref mut inner = *inner_guard;
                unsafe {
                    // Other thread could poll the result, so we check if result has a value
                    if (*self.inner.result.get()).is_some() {
                        should_unpark_tasks = true;
                    } else {
                        match inner.original_future.poll() {
                            Ok(Async::Ready(item)) => {
                                *self.inner.result.get() =
                                    Some(Ok(Async::Ready(SharedItem::new(item))));
                                should_unpark_tasks = true;
                            }
                            Err(error) => {
                                *self.inner.result.get() = Some(Err(SharedError::new(error)));
                                should_unpark_tasks = true;
                            }
                            Ok(Async::NotReady) => {} // Will be handled later
                        }
                    }
                }
            }
            None => {} // Will be handled later
        }

        if should_unpark_tasks {
            self.inner.tasks_unpark_started.store(true, Ordering::Relaxed);
            match self.inner.tasks_receiver.try_lock() {
                Some(tasks_receiver_guard) => {
                    let ref tasks_receiver = *tasks_receiver_guard;
                    loop {
                        match tasks_receiver.try_recv() {
                            Ok(task) => task.unpark(),
                            _ => break,
                        }
                    }
                }
                None => {} // Other thread is unparking the tasks
            }

            return result.clone();
        }

        let t = task::park();
        let _ = self.tasks_sender.send(t);
        if self.inner.tasks_unpark_started.load(Ordering::Relaxed) {
            // If the tasks unpark has started, self.inner.result has a value (not None).
            // The result must be read here because it is possible that the task,
            // t (see variable above), had not been unparked.
            unsafe {
                if let Some(ref result) = *self.inner.result.get() {
                    return result.clone();
                } else {
                    // How should I use unwrap here?
                    // The compiler says cannot "move out of borrowed content"
                    unreachable!();
                }
            }
        }

        Ok(Async::NotReady)
    }
}

impl<F> Clone for Shared<F>
    where F: Future
{
    fn clone(&self) -> Self {
        Shared {
            inner: self.inner.clone(),
            tasks_sender: self.tasks_sender.clone(),
        }
    }
}
