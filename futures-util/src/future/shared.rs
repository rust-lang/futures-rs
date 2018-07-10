//! Definition of the Shared combinator, a future that is cloneable,
//! and can be polled in multiple threads.
//!
//! # Examples
//!
//! ```
//! # extern crate futures;
//! use futures::prelude::*;
//! use futures::future;
//! use futures::executor::block_on;
//!
//! # fn main() {
//! let future = future::ready(6);
//! let shared1 = future.shared();
//! let shared2 = shared1.clone();
//! assert_eq!(6, *block_on(shared1));
//! assert_eq!(6, *block_on(shared2));
//! # }
//! ```

use futures_core::future::Future;
use futures_core::task::{self, Poll, Wake, Waker};
use slab::Slab;
use std::fmt;
use std::cell::UnsafeCell;
use std::marker::Unpin;
use std::mem::PinMut;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::task::local_waker_from_nonlocal;

/// A future that is cloneable and can be polled in multiple threads.
/// Use `Future::shared()` method to convert any future into a `Shared` future.
#[must_use = "futures do nothing unless polled"]
pub struct Shared<Fut: Future> {
    inner: Arc<Inner<Fut>>,
    waker_key: usize,
}

struct Inner<Fut: Future> {
    future_or_output: UnsafeCell<FutureOrOutput<Fut>>,
    notifier: Arc<Notifier>,
}

struct Notifier {
    state: AtomicUsize,
    wakers: Mutex<Option<Slab<Option<Waker>>>>,
}

// The future itself is polled behind the `Arc`, so it won't be moved
// when `Shared` is moved.
impl<Fut: Future> Unpin for Shared<Fut> {}

impl<Fut: Future> fmt::Debug for Shared<Fut> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Shared")
            .field("inner", &self.inner)
            .field("waker_key", &self.waker_key)
            .finish()
    }
}

impl<Fut: Future> fmt::Debug for Inner<Fut> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Inner")
            .finish()
    }
}

enum FutureOrOutput<Fut: Future> {
    Future(Fut),
    Output(Arc<Fut::Output>),
}

unsafe impl<Fut> Send for Inner<Fut>
where
    Fut: Future + Send,
    Fut::Output: Send + Sync,
{}

unsafe impl<Fut> Sync for Inner<Fut>
where
    Fut: Future + Send,
    Fut::Output: Send + Sync,
{}

const IDLE: usize = 0;
const POLLING: usize = 1;
const REPOLL: usize = 2;
const COMPLETE: usize = 3;
const POISONED: usize = 4;

const NULL_WAKER_KEY: usize = usize::max_value();

impl<Fut: Future> Shared<Fut> {
    pub(super) fn new(future: Fut) -> Shared<Fut> {
        Shared {
            inner: Arc::new(Inner {
                future_or_output: UnsafeCell::new(FutureOrOutput::Future(future)),
                notifier: Arc::new(Notifier {
                    state: AtomicUsize::new(IDLE),
                    wakers: Mutex::new(Some(Slab::new())),
                }),
            }),
            waker_key: NULL_WAKER_KEY,
        }
    }
}

impl<Fut> Shared<Fut> where Fut: Future {
    /// If any clone of this `Shared` has completed execution, returns its result immediately
    /// without blocking. Otherwise, returns None without triggering the work represented by
    /// this `Shared`.
    pub fn peek(&self) -> Option<Arc<Fut::Output>> {
        match self.inner.notifier.state.load(SeqCst) {
            COMPLETE => {
                Some(unsafe { self.clone_output() })
            }
            POISONED => panic!("inner future panicked during poll"),
            _ => None,
        }
    }

    /// Registers the current task to receive a wakeup when `Inner` is awoken.
    fn set_waker(&mut self, cx: &mut task::Context) {
        // Acquire the lock first before checking COMPLETE to ensure there
        // isn't a race.
        let mut wakers = self.inner.notifier.wakers.lock().unwrap();
        let wakers = if let Some(wakers) = wakers.as_mut() {
            wakers
        } else {
            // The value is already available, so there's no need to set the waker.
            return
        };
        if self.waker_key == NULL_WAKER_KEY {
            self.waker_key = wakers.insert(Some(cx.waker().clone()));
        } else {
            let waker_slot = &mut wakers[self.waker_key];
            let needs_replacement = if let Some(old_waker) = waker_slot {
                // If there's still an unwoken waker in the slot, only replace
                // if the current one wouldn't wake the same task.
                !old_waker.will_wake(cx.waker())
            } else {
                true
            };
            if needs_replacement {
                *waker_slot = Some(cx.waker().clone());
            }
        }
        debug_assert!(self.waker_key != NULL_WAKER_KEY);
    }

    /// Safety: callers must first ensure that `self.inner.state`
    /// is `COMPLETE`
    unsafe fn clone_output(&self) -> Arc<Fut::Output> {
        if let FutureOrOutput::Output(item) = &*self.inner.future_or_output.get() {
            item.clone()
        } else {
            unreachable!()
        }
    }
}

impl<Fut: Future> Future for Shared<Fut> {
    type Output = Arc<Fut::Output>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let this = &mut *self;

        this.set_waker(cx);

        match this.inner.notifier.state.compare_and_swap(IDLE, POLLING, SeqCst) {
            IDLE => {
                // Lock acquired, fall through
            }
            POLLING | REPOLL => {
                // Another task is currently polling, at this point we just want
                // to ensure that our task handle is currently registered

                return Poll::Pending
            }
            COMPLETE => {
                return unsafe { Poll::Ready(this.clone_output()) };
            }
            POISONED => panic!("inner future panicked during poll"),
            _ => unreachable!(),
        }

        let waker = local_waker_from_nonlocal(this.inner.notifier.clone());
        let mut cx = cx.with_waker(&waker);

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

            let _reset = Reset(&this.inner.notifier.state);

            // Poll the future
            let res = unsafe {
                if let FutureOrOutput::Future(future) = &mut *this.inner.future_or_output.get() {
                    PinMut::new_unchecked(future).poll(&mut cx)
                } else {
                    unreachable!()
                }
            };
            match res {
                Poll::Pending => {
                    // Not ready, try to release the handle
                    match this.inner.notifier.state.compare_and_swap(POLLING, IDLE, SeqCst) {
                        POLLING => {
                            // Success
                            return Poll::Pending;
                        }
                        REPOLL => {
                            // Gotta poll again!
                            let prev = this.inner.notifier.state.swap(POLLING, SeqCst);
                            assert_eq!(prev, REPOLL);
                        }
                        _ => unreachable!(),
                    }
                }
                Poll::Ready(output) => {
                    let output = Arc::new(output);
                    unsafe {
                        *this.inner.future_or_output.get() =
                            FutureOrOutput::Output(output.clone());
                    }

                    // Complete the future
                    let mut lock = this.inner.notifier.wakers.lock().unwrap();
                    this.inner.notifier.state.store(COMPLETE, SeqCst);
                    let wakers = &mut lock.take().unwrap();
                    for (_key, opt_waker) in wakers {
                        if let Some(waker) = opt_waker.take() {
                            waker.wake();
                        }
                    }
                    return Poll::Ready(output);
                }
            }
        }
    }
}

impl<Fut> Clone for Shared<Fut> where Fut: Future {
    fn clone(&self) -> Self {
        Shared {
            inner: self.inner.clone(),
            waker_key: NULL_WAKER_KEY,
        }
    }
}

impl<Fut> Drop for Shared<Fut> where Fut: Future {
    fn drop(&mut self) {
        if self.waker_key != NULL_WAKER_KEY {
            if let Ok(mut wakers) = self.inner.notifier.wakers.lock() {
                if let Some(wakers) = wakers.as_mut() {
                    wakers.remove(self.waker_key);
                }
            }
        }
    }
}

impl Wake for Notifier {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.state.compare_and_swap(POLLING, REPOLL, SeqCst);

        let wakers = &mut *arc_self.wakers.lock().unwrap();
        if let Some(wakers) = wakers.as_mut() {
            for (_key, opt_waker) in wakers {
                if let Some(waker) = opt_waker.take() {
                    waker.wake();
                }
            }
        }
    }
}
