use core::fmt;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};

use super::chain::Chain;

/// Future for the `flatten` combinator.
///
/// This combinator turns a `Future`-of-a-`Future` into a single `Future`.
///
/// This is created by the `Future::flatten` method.
#[must_use = "futures do nothing unless polled"]
pub struct Flatten<A>
    where A: Future,
          A::Output: Future,
{
    state: Chain<A, A::Output, ()>,
}

impl<A> Flatten<A>
    where A: Future,
          A::Output: Future,
{
    unsafe_pinned!(state -> Chain<A, A::Output, ()>);

    pub(super) fn new(future: A) -> Flatten<A> {
        Flatten {
            state: Chain::new(future, ()),
        }
    }
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

impl<A> Future for Flatten<A>
    where A: Future,
          A::Output: Future,
{
    type Output = <A::Output as Future>::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        self.state().poll(cx, |a, ()| a)
    }
}
