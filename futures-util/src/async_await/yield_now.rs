use core::pin::Pin;

use futures_core::future::Future;
use futures_core::task::{LocalWaker, Poll};

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
