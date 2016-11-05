use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::ops::Deref;
use std::sync::RwLock;
use std::sync::mpsc::{channel, Receiver, Sender};
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

/// The data that has to be synced to implement `Shared`, in order to satisfy the `Future` trait's constraints.
struct SyncedInner<F>
    where F: Future
{
    original_future: F, // The original future
    result: Option<Result<Async<SharedItem<F::Item>>, SharedError<F::Error>>>, // The original future result wrapped with `SharedItem`/`SharedError`
    tasks_receiver: Lock<Receiver<Task>>, // When original future is polled and ready, unparks all the tasks in that channel
}

struct Inner<F>
    where F: Future
{
    synced_inner: RwLock<SyncedInner<F>>,
    tasks_unpark_started: AtomicBool,
}

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
            synced_inner: RwLock::new(SyncedInner {
                original_future: future,
                result: None,
                tasks_receiver: Lock::new(tasks_receiver),
            }),
            tasks_unpark_started: AtomicBool::new(false),
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
        let mut polled_result: Option<Result<Async<SharedItem<F::Item>>, SharedError<F::Error>>> =
            None;

        // If the result is ready, just return it
        match self.inner.synced_inner.try_read() {
            Ok(inner_guard) => {
                let ref inner = *inner_guard;
                if let Some(ref result) = inner.result {
                    return result.clone();
                }
            }
            _ => {} // Mutex is locked for write
        }

        // The result was not ready.
        match self.inner.synced_inner.try_write() {
            Ok(mut inner_guard) => {
                let ref mut inner = *inner_guard;

                // By the time that synced_inner was unlocked, other thread could poll the result,
                // so we check if result has a value
                if inner.result.is_some() {
                    polled_result = inner.result.clone();
                } else {
                    match inner.original_future.poll() {
                        Ok(Async::Ready(item)) => {
                            inner.result = Some(Ok(Async::Ready(SharedItem::new(item))));
                            polled_result = inner.result.clone();
                        }
                        Err(error) => {
                            inner.result = Some(Err(SharedError::new(error)));
                            polled_result = inner.result.clone();
                        }
                        Ok(Async::NotReady) => {} // Will be handled later
                    }
                }
            }
            Err(_) => {} // Will be handled later
        }

        if let Some(result) = polled_result {
            match self.inner.synced_inner.try_read() {
                Ok(inner_guard) => {
                    let ref inner = *inner_guard;
                    self.inner.tasks_unpark_started.store(true, Ordering::Relaxed);
                    match inner.tasks_receiver.try_lock() {
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
                _ => {
                    // The mutex is locked for write, after the poll was ready.
                    // The context that locked for write will unpark the tasks.
                }
            }
        }

        let t = task::park();
        let _ = self.tasks_sender.send(t);
        if self.inner.tasks_unpark_started.load(Ordering::Relaxed) {
            // If the tasks unpark has started, synced_inner can be locked for read,
            // and its result has value (not None).
            // The result must be read here because it is possible that the task,
            // t (see variable above), had not been unparked.
            match self.inner.synced_inner.try_read() {
                Ok(inner_guard) => {
                    let ref inner = *inner_guard;
                    return inner.result.as_ref().unwrap().clone();
                }
                _ => {
                    // The mutex is locked for write, so the sent task will be unparked later
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
