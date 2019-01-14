use futures_core::task::{local_waker_from_nonlocal, LocalWaker, Wake};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
