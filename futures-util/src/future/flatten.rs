use core::fmt;

use futures_core::{Future, IntoFuture, Poll};
use futures_core::task;

use super::chain::Chain;

/// Future for the `flatten` combinator.
///
/// This combinator turns a `Future`-of-a-`Future` into a single `Future`.
///
/// This is created by the `Future::flatten` method.
#[must_use = "futures do nothing unless polled"]
pub struct Flatten<A> where A: Future, A::Item: IntoFuture {
    state: Chain<A, <A::Item as IntoFuture>::Future, ()>,
}

impl<A> fmt::Debug for Flatten<A>
    where A: Future + fmt::Debug,
          A::Item: IntoFuture,
          <<A as Future>::Item as IntoFuture>::Future: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Flatten")
            .field("state", &self.state)
            .finish()
    }
}

pub fn new<A>(future: A) -> Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
{
    Flatten {
        state: Chain::new(future, ()),
    }
}

impl<A> Future for Flatten<A>
    where A: Future,
          A::Item: IntoFuture,
          <<A as Future>::Item as IntoFuture>::Error: From<<A as Future>::Error>
{
    type Item = <<A as Future>::Item as IntoFuture>::Item;
    type Error = <<A as Future>::Item as IntoFuture>::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        self.state.poll(cx, |a, ()| {
            let future = a?.into_future();
            Ok(Err(future))
        })
    }
}
