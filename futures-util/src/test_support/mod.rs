mod inert;
pub(crate) use inert::InertExecutor;

mod waker;
pub(crate) use waker::noop_waker;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Convenience method for tests that calls a future's [poll](std::future::Future::poll())
/// method with a Context containing a Waker that does nothing.
pub(crate) fn unwakeable_poll<F: Future + Unpin>(fut: &mut F) -> Poll<F::Output> {
    let noop_waker = noop_waker();
    let mut context = Context::from_waker(&noop_waker);
    let pinned_context = Pin::new(fut);
    pinned_context.poll(&mut context)
}