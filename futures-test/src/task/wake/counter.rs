use futures_core::task::{self, LocalWaker, Wake};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// An implementation of [`Wake`](futures_core::task::Wake) that tracks how many
/// times it has been woken.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures_test::task::{panic_context, wake};
///
/// let wake_counter = wake::Counter::new();
/// let mut cx = panic_context();
/// let cx = &mut cx.with_waker(wake_counter.local_waker());
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
    inner: Arc<Inner>,
    local_waker: LocalWaker,
}

#[derive(Debug)]
struct Inner {
    count: AtomicUsize,
}

impl Counter {
    /// Create a new [`Counter`]
    pub fn new() -> Counter {
        let inner = Arc::new(Inner {
            count: AtomicUsize::new(0),
        });
        Counter {
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

    /// Get the number of times this [`Counter`] has been woken
    pub fn count(&self) -> usize {
        self.inner.count.load(Ordering::SeqCst)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

impl Wake for Inner {
    fn wake(arc_self: &Arc<Self>) {
        arc_self.count.fetch_add(1, Ordering::SeqCst);
    }
}
