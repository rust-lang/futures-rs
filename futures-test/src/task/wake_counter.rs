use futures_core::task::Waker;
use futures_util::task::{self, ArcWake};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Number of times the waker was awoken.
///
/// See [`new_count_waker`] for usage.
#[derive(Debug)]
pub struct AwokenCount {
    inner: Arc<WakerInner>,
}

impl AwokenCount {
    /// Get the current count.
    pub fn get(&self) -> usize {
        self.inner.count.load(Ordering::SeqCst)
    }
}

impl PartialEq<usize> for AwokenCount {
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

#[derive(Debug)]
struct WakerInner {
    count: AtomicUsize,
}

impl ArcWake for WakerInner {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let _ = arc_self.count.fetch_add(1, Ordering::SeqCst);
    }
}

/// Create a new [`Waker`] that counts the number of times it's awoken.
///
/// [`Waker`]: futures_core::task::Waker
///
/// # Examples
///
/// ```
/// use futures_test::task::new_count_waker;
///
/// let (waker, count) = new_count_waker();
///
/// assert_eq!(count, 0);
///
/// waker.wake_by_ref();
/// waker.wake();
///
/// assert_eq!(count, 2);
/// ```
pub fn new_count_waker() -> (Waker, AwokenCount) {
    let inner = Arc::new(WakerInner { count: AtomicUsize::new(0) });
    (task::waker(inner.clone()), AwokenCount { inner })
}
