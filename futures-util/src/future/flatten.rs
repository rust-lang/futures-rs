use super::chain::Chain;
use core::fmt;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

/// Future for the `flatten` combinator.
///
/// This combinator turns a `Future`-of-a-`Future` into a single `Future`.
///
/// This is created by the `Future::flatten` method.
#[must_use = "futures do nothing unless polled"]
pub struct Flatten<Fut>
    where Fut: Future,
          Fut::Output: Future,
{
    state: Chain<Fut, Fut::Output, ()>,
}

impl<Fut> Flatten<Fut>
    where Fut: Future,
          Fut::Output: Future,
{
    unsafe_pinned!(state: Chain<Fut, Fut::Output, ()>);

    pub(super) fn new(future: Fut) -> Flatten<Fut> {
        Flatten {
            state: Chain::new(future, ()),
        }
    }
}

impl<Fut> fmt::Debug for Flatten<Fut>
    where Fut: Future + fmt::Debug,
          Fut::Output: Future + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Flatten")
            .field("state", &self.state)
            .finish()
    }
}

impl<Fut> Future for Flatten<Fut>
    where Fut: Future,
          Fut::Output: Future,
{
    type Output = <Fut::Output as Future>::Output;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        self.state().poll(cx, |a, ()| a)
    }
}
