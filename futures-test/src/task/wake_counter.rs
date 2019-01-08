use futures_core::task::{self, local_waker_from_nonlocal, LocalWaker, Wake};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// An implementation of [`Wake`](futures_core::task::Wake) that tracks how many
/// times it has been woken.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures_test::task::WakeCounter;
///
/// let wake_counter = WakeCounter::new();
/// let lw = wake_counter.local_waker();
///
/// assert_eq!(wake_counter.count(), 0);
///
/// lw.wake();
/// lw.wake();
///
/// assert_eq!(wake_counter.count(), 2);
/// ```
#[derive(Debug)]
pub struct WakeCounter {
    inner: Arc<Inner>,
    local_waker: LocalWaker,
}

#[derive(Debug)]
struct Inner {
    count: AtomicUsize,
}

impl WakeCounter {
    /// Create a new [`WakeCounter`]
    pub fn new() -> WakeCounter {
        let inner = Arc::new(Inner {
            count: AtomicUsize::new(0),
        });
        WakeCounter {
            local_waker: task::local_waker_from_nonlocal(inner.clone()),
            inner,
        }
    }

    /// Creates an associated [`LocalWaker`]. Every call to its
    /// [`wake`](LocalWaker::wake) and
    /// [`wake_local`](LocalWaker::wake) methods increments the counter.
    pub fn local_waker(&self) -> &LocalWaker {
        &self.local_waker
    }

    /// Get the number of times this [`WakeCounter`] has been woken
    pub fn count(&self) -> usize {
        self.inner.count.load(Ordering::SeqCst)
    }
}

impl Default for WakeCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Wake for Inner {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.count.fetch_add(1, Ordering::SeqCst);
    }
}

/// Number of times the waker was awoken.
///
/// See [`new_count_waker`] for usage.
#[derive(Debug)]
pub struct AwokenCount {
    inner: Arc<WakerInner>,
}

impl PartialEq<usize> for AwokenCount {
    fn eq(&self, other: &usize) -> bool {
        self.inner.count.load(Ordering::SeqCst) == *other
    }
}

#[derive(Debug)]
struct WakerInner {
    count: AtomicUsize,
}

impl Wake for WakerInner {
    fn wake(arc_self: &Arc<Self>) {
        let _ = arc_self.count.fetch_add(1, Ordering::SeqCst);
    }
}

/// Create a new [`LocalWaker`] that counts the number of times it's awoken.
///
/// [`LocalWaker`]: futures_core::task::LocalWaker
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures_test::task::new_count_waker;
///
/// let (lw, count) = new_count_waker();
///
/// assert_eq!(count, 0);
///
/// lw.wake();
/// lw.wake();
///
/// assert_eq!(count, 2);
/// ```
pub fn new_count_waker() -> (LocalWaker, AwokenCount) {
    let inner = Arc::new(WakerInner { count: AtomicUsize::new(0) });
    (local_waker_from_nonlocal(inner.clone()), AwokenCount { inner })
}
