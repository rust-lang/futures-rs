use core::fmt;
use core::mem::Pin;

use futures_core::{Async, Poll};
use futures_core::task;

use super::chain::Chain;

/// Async for the `flatten` combinator.
///
/// This combinator turns a `Async`-of-a-`Async` into a single `Async`.
///
/// This is created by the `Async::flatten` method.
#[must_use = "futures do nothing unless polled"]
pub struct Flatten<A> where A: Async, A::Output: Async {
    state: Chain<A, A::Output, ()>,
}

impl<A> fmt::Debug for Flatten<A>
    where A: Async + fmt::Debug,
          A::Output: Async + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Flatten")
            .field("state", &self.state)
            .finish()
    }
}

pub fn new<A>(future: A) -> Flatten<A>
    where A: Async,
          A::Output: Async,
{
    Flatten {
        state: Chain::new(future, ()),
    }
}

impl<A> Async for Flatten<A>
    where A: Async,
          A::Output: Async,
{
    type Output = <A::Output as Async>::Output;

    fn poll(mut self: Pin<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe { pinned_field!(self, state) }.poll(cx, |a, ()| a)
    }
}
