use super::{TryChain, TryChainAction};
use core::mem::PinMut;
use futures_core::{Future, TryFuture};
use futures_core::task::{Context, Poll};

/// Future for the `or_else` combinator, chaining a computation onto the end of
/// a future which fails with an error.
///
/// This is created by the `Future::or_else` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct OrElse<Fut1, Fut2, F> {
    try_chain: TryChain<Fut1, Fut2, F>,
}

impl<Fut1, Fut2, F> OrElse<Fut1, Fut2, F>
    where Fut1: TryFuture,
          Fut2: TryFuture,
{
    unsafe_pinned!(try_chain -> TryChain<Fut1, Fut2, F>);

    /// Creates a new `Then`.
    pub fn new(future: Fut1, async_op: F) -> OrElse<Fut1, Fut2, F> {
        OrElse {
            try_chain: TryChain::new(future, async_op),
        }
    }
}

impl<Fut1, Fut2, F> Future for OrElse<Fut1, Fut2, F>
    where Fut1: TryFuture,
          Fut2: TryFuture<Item = Fut1::Item>,
          F: FnOnce(Fut1::Error) -> Fut2,
{
    type Output = Result<Fut2::Item, Fut2::Error>;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.try_chain().poll(cx, |result, async_op| {
            match result {
                Ok(item) => TryChainAction::Output(Ok(item)),
                Err(err) => TryChainAction::Future(async_op(err)),
            }
        })
    }
}
