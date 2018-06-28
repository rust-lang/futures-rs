//! Await
//!
//! This module contains a number of functions and combinators for working
//! with `async`/`await` code.

use futures_core::{task, Future, Poll};
use std::marker::Unpin;
use std::mem::PinMut;

#[doc(hidden)]
pub fn assert_unpin<T: Future + Unpin>(_: &T) {}

/// A macro which returns the result of polling a future once within the
/// current `async` context.
///
/// This macro is only usable inside of `async` functions, closures, and blocks.
#[macro_export]
macro_rules! poll {
    ($x:expr) => {
        await!($crate::await::poll($x))
    }
}

#[doc(hidden)]
pub fn poll<F: Future + Unpin>(future: F) -> impl Future<Output = Poll<F::Output>> {
    PollOnce { future }
}

#[allow(missing_debug_implementations)]
struct PollOnce<F: Future + Unpin> {
    future: F,
}

impl<F: Future + Unpin> Future for PollOnce<F> {
    type Output = Poll<F::Output>;
    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        Poll::Ready(PinMut::new(&mut self.future).poll(cx))
    }
}

/// A macro which yields to the event loop once.
/// This is similar to returning `Poll::Pending` from a `Future::poll` implementation.
/// If `pending!` is used, the current task should be scheduled to receive a wakeup
/// when it is ready to make progress.
///
/// This macro is only usable inside of `async` functions, closures, and blocks.
#[macro_export]
macro_rules! pending {
    () => {
        await!($crate::await::pending_once())
    }
}

#[doc(hidden)]
pub fn pending_once() -> impl Future<Output = ()> {
    PendingOnce { is_ready: false }
}

#[allow(missing_debug_implementations)]
struct PendingOnce {
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

// Primary export is a macro
mod join;

// Primary export is a macro
mod select;

