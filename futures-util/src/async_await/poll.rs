use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{LocalWaker, Poll};

/// A macro which returns the result of polling a future once within the
/// current `async` context.
///
/// This macro is only usable inside of `async` functions, closures, and blocks.
#[macro_export]
macro_rules! poll {
    ($x:expr) => {
        await!($crate::async_await::poll($x))
    }
}

#[doc(hidden)]
pub fn poll<F: Future + Unpin>(future: F) -> PollOnce<F> {
    PollOnce { future }
}

#[allow(missing_debug_implementations)]
#[doc(hidden)]
pub struct PollOnce<F: Future + Unpin> {
    future: F,
}

impl<F: Future + Unpin> Future for PollOnce<F> {
    type Output = Poll<F::Output>;
    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        Poll::Ready(Pin::new(&mut self.future).poll(lw))
    }
}
