use super::{TryChain, TryChainAction};
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};

/// Future for the `and_then` combinator, chaining a computation onto the end of
/// another future which completes successfully.
///
/// This is created by the `Future::and_then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct AndThen<Fut1, Fut2, F> {
    try_chain: TryChain<Fut1, Fut2, F>,
}

impl<Fut1, Fut2, F> AndThen<Fut1, Fut2, F>
    where Fut1: TryFuture,
          Fut2: TryFuture,
{
    unsafe_pinned!(try_chain: TryChain<Fut1, Fut2, F>);

    /// Creates a new `Then`.
    pub(super) fn new(future: Fut1, async_op: F) -> AndThen<Fut1, Fut2, F> {
        AndThen {
            try_chain: TryChain::new(future, async_op),
        }
    }
}

impl<Fut1, Fut2, F> Future for AndThen<Fut1, Fut2, F>
    where Fut1: TryFuture,
          Fut2: TryFuture<Error = Fut1::Error>,
          F: FnOnce(Fut1::Item) -> Fut2,
{
    type Output = Result<Fut2::Item, Fut2::Error>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        self.try_chain().poll(cx, |result, async_op| {
            match result {
                Ok(item) => TryChainAction::Future(async_op(item)),
                Err(err) => TryChainAction::Output(Err(err)),
            }
        })
    }
}
