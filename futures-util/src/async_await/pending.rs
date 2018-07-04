use futures_core::{task, Future, Poll};
use core::mem::PinMut;

/// A macro which yields to the event loop once.
/// This is similar to returning `Poll::Pending` from a `Future::poll` implementation.
/// If `pending!` is used, the current task should be scheduled to receive a wakeup
/// when it is ready to make progress.
///
/// This macro is only usable inside of `async` functions, closures, and blocks.
#[macro_export]
macro_rules! pending {
    () => {
        await!($crate::async_await::pending_once())
    }
}

#[doc(hidden)]
pub fn pending_once() -> PendingOnce {
    PendingOnce { is_ready: false }
}

#[allow(missing_debug_implementations)]
#[doc(hidden)]
pub struct PendingOnce {
    is_ready: bool,
}

impl Future for PendingOnce {
    type Output = ();
    fn poll(mut self: PinMut<Self>, _: &mut task::Context) -> Poll<Self::Output> {
        if self.is_ready {
            Poll::Ready(())
        } else {
            self.is_ready = true;
            Poll::Pending
        }
    }
}
