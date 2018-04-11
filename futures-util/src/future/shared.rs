//! Definition of the Shared combinator, a future that is cloneable,
//! and can be polled in multiple threads.
//!
//! # Examples
//!
//! ```
//! # extern crate futures;
//! # extern crate futures_executor;
//! use futures::prelude::*;
//! use futures::future;
//! use futures_executor::block_on;
//!
//! # fn main() {
//! let future = future::ok::<_, bool>(6);
//! let shared1 = future.shared();
//! let shared2 = shared1.clone();
//! assert_eq!(6, *block_on(shared1).unwrap());
//! assert_eq!(6, *block_on(shared2).unwrap());
//! # }
//! ```

use std::{error, fmt, mem, ops};
use std::cell::UnsafeCell;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::collections::HashMap;

use futures_core::{Future, Poll, Async};
use futures_core::task::{self, Wake, Waker, LocalMap};

/// A future that is cloneable and can be polled in multiple threads.
/// Use `Future::shared()` method to convert any future into a `Shared` future.
#[must_use = "futures do nothing unless polled"]
pub struct Shared<F: Future> {
    inner: Arc<Inner<F>>,
    waiter: usize,
}

impl<F> fmt::Debug for Shared<F>
    where F: Future + fmt::Debug,
          F::Item: fmt::Debug,
          F::Error: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Shared")
            .field("inner", &self.inner)
            .field("waiter", &self.waiter)
            .finish()
    }
}

struct Inner<F: Future> {
    next_clone_id: AtomicUsize,
    future: UnsafeCell<Option<(F, LocalMap)>>,
    result: UnsafeCell<Option<Result<SharedItem<F::Item>, SharedError<F::Error>>>>,
    notifier: Arc<Notifier>,
}

struct Notifier {
    state: AtomicUsize,
    waiters: Mutex<HashMap<usize, Waker>>,
}

const IDLE: usize = 0;
const POLLING: usize = 1;
const REPOLL: usize = 2;
const COMPLETE: usize = 3;
const POISONED: usize = 4;

pub fn new<F: Future>(future: F) -> Shared<F> {
    Shared {
        inner: Arc::new(Inner {
            next_clone_id: AtomicUsize::new(1),
            notifier: Arc::new(Notifier {
                state: AtomicUsize::new(IDLE),
                waiters: Mutex::new(HashMap::new()),
            }),
            future: UnsafeCell::new(Some((future, LocalMap::new()))),
            result: UnsafeCell::new(None),
        }),
        waiter: 0,
    }
}

impl<F> Shared<F> where F: Future {
    /// If any clone of this `Shared` has completed execution, returns its result immediately
    /// without blocking. Otherwise, returns None without triggering the work represented by
    /// this `Shared`.
    pub fn peek(&self) -> Option<Result<SharedItem<F::Item>, SharedError<F::Error>>> {
        match self.inner.notifier.state.load(SeqCst) {
            COMPLETE => {
                Some(unsafe { self.clone_result() })
            }
            POISONED => panic!("inner future panicked during poll"),
            _ => None,
        }
    }

    fn set_waiter(&mut self, cx: &mut task::Context) {
        let mut waiters = self.inner.notifier.waiters.lock().unwrap();
        waiters.insert(self.waiter, cx.waker().clone());
    }

    unsafe fn clone_result(&self) -> Result<SharedItem<F::Item>, SharedError<F::Error>> {
        match *self.inner.result.get() {
            Some(Ok(ref item)) => Ok(SharedItem { item: item.item.clone() }),
            Some(Err(ref e)) => Err(SharedError { error: e.error.clone() }),
            _ => unreachable!(),
        }
    }

    fn complete(&self) {
        unsafe { *self.inner.future.get() = None };
        self.inner.notifier.state.store(COMPLETE, SeqCst);
        Wake::wake(&self.inner.notifier);
    }
}

impl<F> Future for Shared<F>
    where F: Future
{
    type Item = SharedItem<F::Item>;
    type Error = SharedError<F::Error>;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        self.set_waiter(cx);

        match self.inner.notifier.state.compare_and_swap(IDLE, POLLING, SeqCst) {
            IDLE => {
                // Lock acquired, fall through
            }
            POLLING | REPOLL => {
                // Another task is currently polling, at this point we just want
                // to ensure that our task handle is currently registered

                return Ok(Async::Pending);
            }
            COMPLETE => {
                return unsafe { self.clone_result().map(Async::Ready) };
            }
            POISONED => panic!("inner future panicked during poll"),
            _ => unreachable!(),
        }

        loop {
            struct Reset<'a>(&'a AtomicUsize);

            impl<'a> Drop for Reset<'a> {
                fn drop(&mut self) {
                    use std::thread;

                    if thread::panicking() {
                        self.0.store(POISONED, SeqCst);
                    }
                }
            }

            let _reset = Reset(&self.inner.notifier.state);

            // Poll the future
            let res = unsafe {
                let (ref mut f, ref mut data) = *(*self.inner.future.get()).as_mut().unwrap();
                let waker = Waker::from(self.inner.notifier.clone());
                let mut cx = task::Context::new(data, &waker, cx.executor());
                f.poll(&mut cx)
            };
            match res {
                Ok(Async::Pending) => {
                    // Not ready, try to release the handle
                    match self.inner.notifier.state.compare_and_swap(POLLING, IDLE, SeqCst) {
                        POLLING => {
                            // Success
                            return Ok(Async::Pending);
                        }
                        REPOLL => {
                            // Gotta poll again!
                            let prev = self.inner.notifier.state.swap(POLLING, SeqCst);
                            assert_eq!(prev, REPOLL);
                        }
                        _ => unreachable!(),
                    }

                }
                Ok(Async::Ready(i)) => {
                    unsafe {
                        (*self.inner.result.get()) = Some(Ok(SharedItem { item: Arc::new(i) }));
                    }

                    break;
                }
                Err(e) => {
                    unsafe {
                        (*self.inner.result.get()) = Some(Err(SharedError { error: Arc::new(e) }));
                    }

                    break;
                }
            }
        }

        self.complete();
        unsafe { self.clone_result().map(Async::Ready) }
    }
}

impl<F> Clone for Shared<F> where F: Future {
    fn clone(&self) -> Self {
        let next_clone_id = self.inner.next_clone_id.fetch_add(1, SeqCst);

        Shared {
            inner: self.inner.clone(),
            waiter: next_clone_id,
        }
    }
}

impl<F> Drop for Shared<F> where F: Future {
    fn drop(&mut self) {
        let mut waiters = self.inner.notifier.waiters.lock().unwrap();
        waiters.remove(&self.waiter);
    }
}

impl Wake for Notifier {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.state.compare_and_swap(POLLING, REPOLL, SeqCst);

        let waiters = mem::replace(&mut *arc_self.waiters.lock().unwrap(), HashMap::new());

        for (_, waiter) in waiters {
            waiter.wake();
        }
    }
}

unsafe impl<F: Future> Sync for Inner<F> {}
unsafe impl<F: Future> Send for Inner<F> {}

impl<F> fmt::Debug for Inner<F>
    where F: Future + fmt::Debug,
          F::Item: fmt::Debug,
          F::Error: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Inner")
            .finish()
    }
}

/// A wrapped item of the original future that is cloneable and implements Deref
/// for ease of use.
#[derive(Clone, Debug)]
pub struct SharedItem<T> {
    item: Arc<T>,
}

impl<T> ops::Deref for SharedItem<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.item.as_ref()
    }
}

/// A wrapped error of the original future that is cloneable and implements Deref
/// for ease of use.
#[derive(Clone, Debug)]
pub struct SharedError<E> {
    error: Arc<E>,
}

impl<E> ops::Deref for SharedError<E> {
    type Target = E;

    fn deref(&self) -> &E {
        &self.error.as_ref()
    }
}

impl<E> fmt::Display for SharedError<E>
    where E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl<E> error::Error for SharedError<E>
    where E: error::Error,
{
    fn description(&self) -> &str {
        self.error.description()
    }

    fn cause(&self) -> Option<&error::Error> {
        self.error.cause()
    }
}
