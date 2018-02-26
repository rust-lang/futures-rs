use {Future, Poll, Async};
use task::AtomicTask;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A future which can be cancelled using a `KillHandle`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Killable<T> {
    future: T,
    inner: Arc<KillInner>,
}

impl<T> Killable<T> where T: Future {
    /// Creates a new `Killable` future using an existing `KillRegistration`.
    /// `KillRegistration`s can be acquired through `KillHandle::new`.
    ///
    /// When `kill` is called on `handle`, this future will complete
    /// immediately. If `kill` has already been called, the future
    /// will complete immediately without making any further progress.
    pub fn new(future: T, reg: KillRegistration) -> Self {
        Killable {
            future: future,
            inner: reg.inner,
        }
    }
}

/// A registration handle for a `Killable` future.
/// Values of this type can be acquired from `KillHandle::new` and are used
/// in calls to `Killable::new`.
#[derive(Debug, Clone)]
pub struct KillRegistration {
    inner: Arc<KillInner>,
}

/// A handle to a `Killable` future.
#[derive(Debug, Clone)]
pub struct KillHandle {
    inner: Arc<KillInner>,
}

impl KillHandle {
    /// Creates a new `KillHandle` which can be used to kill a running future.
    /// This function also returns a `KillRegistration` which is used to
    /// pair the handle with a `Killable` future.
    ///
    /// This function is usually paired with a call to `Killable::new`.
    pub fn new() -> (Self, KillRegistration) {
        let inner = Arc::new(KillInner {
            task: AtomicTask::new(),
            cancel: AtomicBool::new(false),
        });

        (
            KillHandle {
                inner: inner.clone(),
            },
            KillRegistration {
                inner: inner,
            },
        )
    }
}

// Inner type storing the task to awaken and a bool indicating that it
// should be cancelled.
#[derive(Debug)]
struct KillInner {
    task: AtomicTask,
    cancel: AtomicBool,
}

/// Creates a new `Killable` future and a `KillHandle` which can be used to stop it.
///
/// This function is a convenient (but less flexible) alternative to calling 
/// `KillHandle::new` and `Killable::new` manually.
pub fn killable<T>(future: T) -> (Killable<T>, KillHandle)
    where T: Future
{
    let (handle, reg) = KillHandle::new();
    (
        Killable::new(future, reg),
        handle,
    )
}

/// Indicator that the `Killable` future was killed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Killed;

impl<T> Future for Killable<T> where T: Future {
    type Item = Result<T::Item, Killed>;
    type Error = T::Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.inner.cancel.load(Ordering::Acquire) {
            Ok(Async::Ready(Err(Killed)))
        } else {
            match self.future.poll() {
                Ok(Async::NotReady) => {
                    self.inner.task.register();
                    Ok(Async::NotReady)
                },
                Ok(Async::Ready(x)) => Ok(Async::Ready(Ok(x))),
                Err(e) => Err(e),
            }
        }
    }
}

impl KillHandle {
    /// Kill the `Killable` future associated with this handle.
    pub fn kill(&self) {
        self.inner.cancel.store(true, Ordering::Release);
        self.inner.task.notify();
    }
}
