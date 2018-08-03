use futures_core::task::{local_waker, LocalWaker, Wake};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// An implementation of [`Wake`][futures_core::task::Wake] that tracks how many
/// times it has been woken.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures_test::task::{panic_context, wake};
///
/// let (wake_counter, local_waker) = wake::Counter::new();
/// let mut cx = panic_context();
/// let cx = &mut cx.with_waker(&local_waker);
///
/// assert_eq!(wake_counter.count(), 0);
///
/// cx.waker().wake();
/// cx.waker().wake();
///
/// assert_eq!(wake_counter.count(), 2);
/// ```
#[derive(Debug)]
pub struct Counter {
    count: AtomicUsize,
}

impl Counter {
    /// Create a new instance with an associated [`LocalWaker`]
    pub fn new() -> (Arc<Self>, LocalWaker) {
        let arc = Arc::new(Self {
            count: AtomicUsize::new(0),
        });
        let local_waker = unsafe { local_waker(arc.clone()) };
        (arc, local_waker)
    }

    /// Get the number of times this [`Counter`] has been woken
    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self {
            count: AtomicUsize::new(0),
        }
    }
}

impl Wake for Counter {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.count.fetch_add(1, Ordering::SeqCst);
    }
}
