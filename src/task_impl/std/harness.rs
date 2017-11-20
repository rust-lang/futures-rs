use {Future, Poll, Async};
use super::ThreadNotify;
use executor::{spawn, Spawn};

use std::time::{Duration, Instant};

/// Wraps a future, providing an API to interact with it while off task.
///
/// This wrapper is intended to use from the context of tests. The future is
/// effectively wrapped by a task and this harness tracks received notifications
/// as well as provides APIs to perform non-blocking polling as well blocking
/// polling with or without timeout.
#[derive(Debug)]
pub struct TestHarness<T> {
    spawn: Spawn<T>,
}

/// Error produced by `TestHarness` operations with timeout.
#[derive(Debug)]
pub struct TimeoutError<T> {
    /// If `None`, represents a timeout error
    inner: Option<T>,
}

impl<T> TestHarness<T> {
    /// Wraps `obj` in a test harness, enabling interacting with the future
    /// while not on a `Task`.
    pub fn new(obj: T) -> Self {
        TestHarness {
            spawn: spawn(obj),
        }
    }

    /// Returns `true` if the inner future has received a readiness notification
    /// since the last action has been performed.
    pub fn is_notified(&self) -> bool {
        ThreadNotify::with_current(|notify| notify.is_notified())
    }

    /// Returns a reference to the inner future.
    pub fn get_ref(&self) -> &T {
        self.spawn.get_ref()
    }

    /// Returns a mutable reference to the inner future.
    pub fn get_mut(&mut self) -> &mut T {
        self.spawn.get_mut()
    }

    /// Consumes `self`, returning the inner future.
    pub fn into_inner(self) -> T {
        self.spawn.into_inner()
    }
}

impl<T: Future> TestHarness<T> {
    /// Polls the inner future.
    ///
    /// This function returns immediately. If the inner future is not currently
    /// ready, `NotReady` is returned. Readiness notifications are tracked and
    /// can be queried using `is_notified`.
    pub fn poll(&mut self) -> Poll<T::Item, T::Error> {
        ThreadNotify::with_current(|notify| {
            self.spawn.poll_future_notify(notify, 0)
        })
    }

    /// Waits for the internal future to complete, blocking this thread's
    /// execution until it does.
    pub fn wait(&mut self) -> Result<T::Item, T::Error> {
        self.spawn.wait_future()
    }

    /// Waits for the internal future to complete, blocking this thread's
    /// execution for at most `dur`.
    pub fn wait_timeout(&mut self, dur: Duration)
        -> Result<T::Item, TimeoutError<T::Error>>
    {
        let until = Instant::now() + dur;

        ThreadNotify::with_current(|notify| {
            notify.clear();

            loop {
                let res = self.spawn.poll_future_notify(notify, 0)
                    .map_err(TimeoutError::new);

                match res? {
                    Async::NotReady => {
                        let now = Instant::now();

                        if now >= until {
                            return Err(TimeoutError::timeout());
                        }

                        notify.park_timeout(Some(until - now));
                    }
                    Async::Ready(e) => return Ok(e),
                }
            }
        })
    }
}

impl<T> TimeoutError<T> {
    fn new(inner: T) -> Self {
        TimeoutError { inner: Some(inner) }
    }

    fn timeout() -> Self {
        TimeoutError { inner: None }
    }

    pub fn is_timeout(&self) -> bool {
        self.inner.is_none()
    }

    /// Consumes `self`, returning the inner error. Returns `None` if `self`
    /// represents a timeout.
    pub fn into_inner(self) -> Option<T> {
        self.inner
    }
}
