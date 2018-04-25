use core::mem::Pin;

use futures_core::{Async, Poll};
use futures_core::task;
use super::chain::Chain;

/// Async for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by the `Async::then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Then<A, B, F> where A: Async, B: Async {
    state: Chain<A, B, F>,
}

pub fn new<A, B, F>(future: A, f: F) -> Then<A, B, F>
    where A: Async,
          B: Async,
{
    Then {
        state: Chain::new(future, f),
    }
}

impl<A, B, F> Async for Then<A, B, F>
    where A: Async,
          B: Async,
          F: FnOnce(A::Output) -> B,
{
    type Output = B::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<B::Output> {
        unsafe { pinned_field!(self, state) }.poll(cx, |a, f| f(a))
    }
}
