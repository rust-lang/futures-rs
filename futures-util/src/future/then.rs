use core::mem::Pin;

use futures_core::{Future, Poll};
use futures_core::task;
use super::chain::Chain;

/// Future for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by the `Future::then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Then<A, B, F> where A: Future, B: Future {
    state: Chain<A, B, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F>
    where A: Future,
          B: Future,
{
    Then {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Future for Then<A, B, F>
    where A: Future,
          B: Future,
          F: FnOnce(A::Output) -> B,
{
    type Output = B::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<B::Output> {
        unsafe { pinned_field!(self, state) }.poll(cx, |a, f| f(a))
    }
}
