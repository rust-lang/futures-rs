use core::pin::Pin;

use futures_core::future::Future;
use futures_core::task::{LocalWaker, Poll};

/// A macro which temporarily give up the CPU
///
/// # What's the difference between yield_now!() and pending!()?
///
/// When using the [`pending!()`](pending!), it must be ensured that [`wake`](std::task::Waker::wake)
/// is called somewhere when further progress can be made.
///
/// `yield_now!()` just like [`std::thread::yield_now`](std::thread::yield_now), but it used for asynchronous programming.
///
/// # Examples
///
/// ```
/// #![feature(await_macro, async_await, futures_api)]
///
/// use futures::yield_now;
///
/// # let mut count = 10usize;
/// # let mut is_ready = || {
/// #    count -= 1;
/// #    count == 0
/// # };
/// async {
///     loop {
///         if is_ready() {
///             // finish job
///             break;
///         } else {
///             // do something
///             yield_now!();
///         }
///     }
/// };
///
/// ```
///
/// This macro is only usable inside of async functions, closures, and blocks.
#[macro_export]
macro_rules! yield_now {
    () => {
        await!($crate::async_await::yield_now())
    };
}

#[doc(hidden)]
pub fn yield_now() -> impl Future<Output=()> {
    YieldNow { is_ready: false }
}

#[allow(missing_debug_implementations)]
struct YieldNow {
    is_ready: bool,
}

impl Future for YieldNow {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        if self.is_ready {
            Poll::Ready(())
        } else {
            self.is_ready = true;
            lw.wake();
            Poll::Pending
        }
    }
}
