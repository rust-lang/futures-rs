use core::fmt;
use core::mem::Pin;

use futures_core::{Future, Poll};
use futures_core::task;

use super::chain::Chain;

/// Future for the `flatten` combinator.
///
/// This combinator turns a `Future`-of-a-`Future` into a single `Future`.
///
/// This is created by the `Future::flatten` method.
#[must_use = "futures do nothing unless polled"]
pub struct Flatten<A> where A: Future, A::Output: Future {
    state: Chain<A, A::Output, ()>,
}

impl<A> fmt::Debug for Flatten<A>
    where A: Future + fmt::Debug,
          A::Output: Future + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Flatten")
            .field("state", &self.state)
            .finish()
    }
}

pub fn new<A>(future: A) -> Flatten<A>
    where A: Future,
          A::Output: Future,
{
    Flatten {
        state: Chain::new(future, ()),
    }
}

impl<A> Future for Flatten<A>
    where A: Future,
          A::Output: Future,
{
    type Output = <A::Output as Future>::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe { pinned_field!(self, state) }.poll(cx, |a, ()| a)
    }
}
