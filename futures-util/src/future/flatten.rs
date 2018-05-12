use core::fmt;

use futures_core::{Future, Poll};
use futures_core::task;

use super::chain::Chain;

/// Future for the `flatten` combinator.
///
/// This combinator turns a `Future`-of-a-`Future` into a single `Future`.
///
/// This is created by the `Future::flatten` method.
#[must_use = "futures do nothing unless polled"]
pub struct Flatten<A: Future> {
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

#[cfg(feature = "nightly")]
impl<A> Future for Flatten<A>
    where A: Future,
          A::Output: Future,
{
    type Output = <A::Output as Future>::Output;

    fn poll(mut self: ::core::mem::PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        unsafe { pinned_field!(self, state) }.poll(cx, |a, ()| a)
    }
}

#[cfg(not(feature = "nightly"))]
impl<A> Future for Flatten<A>
    where A: Future + ::futures_core::Unpin,
          A::Output: Future + ::futures_core::Unpin,
{
    type Output = <A::Output as Future>::Output;

    fn poll_unpin(&mut self, cx: &mut task::Context) -> Poll<Self::Output> {
        self.state.poll_unpin(cx, |a, ()| a)
    }

    unpinned_poll!();
}

#[cfg(not(feature = "nightly"))]
unsafe impl<A: Future> ::futures_core::Unpin for Flatten<A> {}
