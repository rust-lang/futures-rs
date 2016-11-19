use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::ops::Deref;
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

struct SyncedInner<F>
    where F: Future
{
    /// The original future that is wrapped as `Shared`
    original_future: F,
    /// When original future is polled and ready, all the tasks in that channel will be unparked
    tasks_receiver: Receiver<Task>,
}

struct Inner<F>
    where F: Future
{
    /// The original future and the tasks receiver behind a mutex
    synced_inner: Lock<SyncedInner<F>>,
    /// Indicates whether the result is ready, and the tasks unparking has been started
    tasks_unpark_started: AtomicBool,
    /// The original future result wrapped with `SharedItem`/`SharedError`
    result: RwLock<Option<Result<SharedItem<F::Item>, SharedError<F::Error>>>>,
}


/// TODO: doc
#[must_use = "futures do nothing unless polled"]
pub struct Shared<F>
    where F: Future
{
    inner: Arc<Inner<F>>,
    tasks_sender: Sender<Task>,
}

impl<F> Shared<F>
    where F: Future
{
    fn result_to_polled_result(result: Result<SharedItem<F::Item>, SharedError<F::Error>>)
                               -> Result<Async<SharedItem<F::Item>>, SharedError<F::Error>> {
        match result {
            Ok(item) => Ok(Async::Ready(item)),
            Err(error) => Err(error),
        }
    }

    fn unpark_tasks(&self, tasks_receiver: &mut Receiver<Task>) {
        self.inner.tasks_unpark_started.store(true, Ordering::Relaxed);
        loop {
            match tasks_receiver.try_recv() {
                Ok(task) => task.unpark(),
                _ => break,
            }
        }
    }
}

pub fn new<F>(future: F) -> Shared<F>
    where F: Future
{
    let (tasks_sender, tasks_receiver) = channel();
    Shared {
        inner: Arc::new(Inner {
            synced_inner: Lock::new(SyncedInner {
                original_future: future,
                tasks_receiver: tasks_receiver,
            }),
            tasks_unpark_started: AtomicBool::new(false),
            result: RwLock::new(None),
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
        // 2. Try lock the self.inner.original_future:
        //    - If successfully locked, poll the original future.
        //      If the future is ready, unpark the tasks from
        //      self.inner.tasks_receiver and return the result.
        //    - If the future is not ready:
        // 3. Create a task and send it through self.tasks_sender.
        // 4. Check again if the result is ready (with tasks_unpark_started)
        // 5. Return the result if it's ready. It is necessary because otherwise there could be
        //    a race between the task sending and the thread receiving the tasks.

        // If the result is ready, just return it
        if self.inner.tasks_unpark_started.load(Ordering::Relaxed) {
            return Self::result_to_polled_result(self.inner
                .result
                .read()
                .unwrap()
                .clone()
                .unwrap());
        }

        // The result was not ready.
        // Try lock the original future.
        match self.inner.synced_inner.try_lock() {
            Some(mut synced_inner) => {
                let ref mut synced_inner = *synced_inner;
                let ref mut original_future = synced_inner.original_future;
                let mut result = self.inner.result.write().unwrap();
                // Other thread could already poll the result, so we check if result has a value
                if result.is_none() {
                    match original_future.poll() {
                        Ok(Async::Ready(item)) => {
                            *result = Some(Ok(SharedItem::new(item)));
                            self.unpark_tasks(&mut synced_inner.tasks_receiver);
                            return Self::result_to_polled_result(result.clone().unwrap());
                        }
                        Err(error) => {
                            *result = Some(Err(SharedError::new(error)));
                            self.unpark_tasks(&mut synced_inner.tasks_receiver);
                            return Self::result_to_polled_result(result.clone().unwrap());
                        }
                        Ok(Async::NotReady) => {} // A task will be parked
                    }
                }
            }
            None => {} // A task will be parked
        }

        let t = task::park();
        let _ = self.tasks_sender.send(t);
        if self.inner.tasks_unpark_started.load(Ordering::Relaxed) {
            // If the tasks unpark has been started, self.inner.result has a value (not None).
            // The result must be read now even after sending the parked task
            // because it is possible that the task, t (see variable above),
            // had not been unparked.
            // That's because self.inner.tasks_receiver could has tasks that will never be unparked,
            // because the tasks receiving loop could have ended before receiving
            // the new task, t).
            return Self::result_to_polled_result(self.inner
                .result
                .read()
                .unwrap()
                .clone()
                .unwrap());
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
