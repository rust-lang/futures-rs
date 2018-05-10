use futures_core::{Future, Poll};
use futures_core::task;
use super::chain::Chain;

/// Future for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by the `Future::then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Then<A, B, F>  {
    state: Chain<A, B, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F> {
    Then {
        state: Chain::new(future, f),
    }
}

#[cfg(feature = "nightly")]
impl<A, B, F> Future for Then<A, B, F>
    where A: Future, B: Future, F: FnOnce(A::Output) -> B
{
    type Output = B::Output;

    fn poll(mut self: ::core::mem::PinMut<Self>, cx: &mut task::Context) -> Poll<B::Output> {
        unsafe { pinned_field!(self, state) }.poll(cx, |a, f| f(a))
    }
}

#[cfg(not(feature = "nightly"))]
impl<A, B, F> Future for Then<A, B, F>
    where A: Future + ::futures_core::Unpin,
          B: Future + ::futures_core::Unpin,
          F: FnOnce(A::Output) -> B
{
    type Output = B::Output;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<B::Output> {
        self.state.poll_unpin(cx, |a, f| f(a))
    }

    unpinned_poll!();
}

#[cfg(not(feature = "nightly"))]
unsafe impl<A, B, F> ::futures_core::Unpin for Then<A, B, F> {}
