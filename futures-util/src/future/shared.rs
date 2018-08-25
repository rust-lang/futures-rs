use futures_core::future::Future;
use futures_core::task::{LocalWaker, Poll, Wake, Waker};
use slab::Slab;
use std::fmt;
use std::cell::UnsafeCell;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::task::local_waker_from_nonlocal;

/// A future that is cloneable and can be polled in multiple threads.
/// Use `Future::shared()` method to convert any future into a `Shared` future.
#[must_use = "futures do nothing unless polled"]
pub struct Shared<Fut: Future> {
    inner: Option<Arc<Inner<Fut>>>,
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
    Output(Fut::Output),
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
            inner: Some(Arc::new(Inner {
                future_or_output: UnsafeCell::new(FutureOrOutput::Future(future)),
                notifier: Arc::new(Notifier {
                    state: AtomicUsize::new(IDLE),
                    wakers: Mutex::new(Some(Slab::new())),
                }),
            })),
            waker_key: NULL_WAKER_KEY,
        }
    }
}

impl<Fut> Shared<Fut>
where
    Fut: Future,
    Fut::Output: Clone,
{
    /// Returns Some containing a reference to this `Shared`'s output if it has
    /// already been computed by a clone or `None` if it hasn't been computed yet
    /// or if this `Shared` already returned its output from poll.
    pub fn peek(&self) -> Option<&Fut::Output> {
        match self.inner.as_ref().map(|inner| inner.notifier.state.load(SeqCst)) {
            Some(COMPLETE) => unsafe { Some(self.inner().get_output()) },
            Some(POISONED) => panic!("inner future panicked during poll"),
            _ => None,
        }
    }

    fn inner(&self) -> &Arc<Inner<Fut>> {
        Self::inner_(&self.inner)
    }

    fn inner_(inner: &Option<Arc<Inner<Fut>>>) -> &Arc<Inner<Fut>> {
        inner.as_ref().expect("Shared future polled again after completion")
    }

    /// Registers the current task to receive a wakeup when `Inner` is awoken.
    fn set_waker(&mut self, lw: &LocalWaker) {
        // Acquire the lock first before checking COMPLETE to ensure there
        // isn't a race.
        let mut wakers = Self::inner_(&self.inner).notifier.wakers.lock().unwrap();
        let wakers = if let Some(wakers) = wakers.as_mut() {
            wakers
        } else {
            // The value is already available, so there's no need to set the waker.
            return
        };
        if self.waker_key == NULL_WAKER_KEY {
            self.waker_key = wakers.insert(Some(lw.clone().into_waker()));
        } else {
            let waker_slot = &mut wakers[self.waker_key];
            let needs_replacement = if let Some(old_waker) = waker_slot {
                // If there's still an unwoken waker in the slot, only replace
                // if the current one wouldn't wake the same task.
                !lw.will_wake_nonlocal(old_waker)
            } else {
                true
            };
            if needs_replacement {
                *waker_slot = Some(lw.clone().into_waker());
            }
        }
        debug_assert!(self.waker_key != NULL_WAKER_KEY);
    }

    /// Safety: callers must first ensure that `self.inner.state`
    /// is `COMPLETE`
    unsafe fn take_or_clone_output(inner: Arc<Inner<Fut>>) -> Fut::Output {
        match Arc::try_unwrap(inner) {
            Ok(inner) => {
                match inner.future_or_output.into_inner() {
                    FutureOrOutput::Output(item) => item,
                    FutureOrOutput::Future(_) => unreachable!(),
                }
            }
            Err(inner) => inner.get_output().clone(),
        }
    }

}


impl<Fut> Inner<Fut>
where
    Fut: Future,
    Fut::Output: Clone,
{
    /// Safety: callers must first ensure that `self.inner.state`
    /// is `COMPLETE`
    unsafe fn get_output(&self) -> &Fut::Output {
        match &*self.future_or_output.get() {
            FutureOrOutput::Output(ref item) => &item,
            FutureOrOutput::Future(_) => unreachable!(),
        }
    }
}

impl<Fut: Future> Future for Shared<Fut>
where
    Fut::Output: Clone,
{
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let this = &mut *self;

        // Assert that we aren't completed
        this.inner();

        this.set_waker(lw);

        match this.inner().notifier.state.compare_and_swap(IDLE, POLLING, SeqCst) {
            IDLE => {
                // Lock acquired, fall through
            }
            POLLING | REPOLL => {
                // Another task is currently polling, at this point we just want
                // to ensure that our task handle is currently registered

                return Poll::Pending;
            }
            COMPLETE => {
                let inner = self.inner.take().unwrap();
                return unsafe { Poll::Ready(Self::take_or_clone_output(inner)) };
            }
            POISONED => panic!("inner future panicked during poll"),
            _ => unreachable!(),
        }

        let waker = local_waker_from_nonlocal(this.inner().notifier.clone());
        let lw = &waker;

        struct Reset<'a>(&'a AtomicUsize);

        impl<'a> Drop for Reset<'a> {
            fn drop(&mut self) {
                use std::thread;

                if thread::panicking() {
                    self.0.store(POISONED, SeqCst);
                }
            }
        }


        let output = loop {
            let inner = this.inner();
            let _reset = Reset(&inner.notifier.state);

            // Poll the future
            let res = unsafe {
                if let FutureOrOutput::Future(future) =
                    &mut *inner.future_or_output.get()
                {
                    Pin::new_unchecked(future).poll(lw)
                } else {
                    unreachable!()
                }
            };
            match res {
                Poll::Pending => {
                    // Not ready, try to release the handle
                    match inner
                        .notifier
                        .state
                        .compare_and_swap(POLLING, IDLE, SeqCst)
                    {
                        POLLING => {
                            // Success
                            return Poll::Pending;
                        }
                        REPOLL => {
                            // Gotta poll again!
                            let prev = inner.notifier.state.swap(POLLING, SeqCst);
                            assert_eq!(prev, REPOLL);
                        }
                        _ => unreachable!(),
                    }
                }
                Poll::Ready(output) => break output,
            }
        };

        if Arc::get_mut(this.inner.as_mut().unwrap()).is_some() {
            this.inner.take();
            return Poll::Ready(output);
        }

        let inner = this.inner();

        let _reset = Reset(&inner.notifier.state);

        unsafe {
            *inner.future_or_output.get() =
                FutureOrOutput::Output(output.clone());
        }

        // Complete the future
        let mut lock = inner.notifier.wakers.lock().unwrap();
        inner.notifier.state.store(COMPLETE, SeqCst);
        let wakers = &mut lock.take().unwrap();
        for (_key, opt_waker) in wakers {
            if let Some(waker) = opt_waker.take() {
                waker.wake();
            }
        }

        Poll::Ready(output)
    }
}

impl<Fut> Clone for Shared<Fut>
where
    Fut: Future,
{
    fn clone(&self) -> Self {
        Shared {
            inner: self.inner.clone(),
            waker_key: NULL_WAKER_KEY,
        }
    }
}

impl<Fut> Drop for Shared<Fut>
where
    Fut: Future,
{
    fn drop(&mut self) {
        if self.waker_key != NULL_WAKER_KEY {
            if let Some(ref inner) = self.inner {
                if let Ok(mut wakers) = inner.notifier.wakers.lock() {
                    if let Some(wakers) = wakers.as_mut() {
                        wakers.remove(self.waker_key);
                    }
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
