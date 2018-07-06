use core::mem::PinMut;
use futures_core::TryFuture;
use futures_core::task::{Context, Poll};

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
crate enum TryChain<Fut1, Fut2, Data> {
    First(Fut1, Option<Data>),
    Second(Fut2),
    Empty,
}

crate enum TryChainAction<Fut2>
    where Fut2: TryFuture,
{
    Future(Fut2),
    Output(Result<Fut2::Item, Fut2::Error>),
}

impl<Fut1, Fut2, Data> TryChain<Fut1, Fut2, Data>
    where Fut1: TryFuture,
          Fut2: TryFuture,
{
    pub fn new(fut1: Fut1, data: Data) -> TryChain<Fut1, Fut2, Data> {
        TryChain::First(fut1, Some(data))
    }

    pub fn poll<F>(
        self: PinMut<Self>,
        cx: &mut Context,
        op: F,
    ) -> Poll<Result<Fut2::Item, Fut2::Error>>
        where F: FnOnce(Result<Fut1::Item, Fut1::Error>, Data) -> TryChainAction<Fut2>,
    {
        let mut op = Some(op);

        // Safe to call `get_mut_unchecked` because we won't move the futures.
        let this = unsafe { PinMut::get_mut_unchecked(self) };

        loop {
            let (output, data) = match this {
                TryChain::First(fut1, data) => {
                    // Poll the first future
                    match unsafe { PinMut::new_unchecked(fut1) }.try_poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(output) => (output, data.take().unwrap()),
                    }
                }
                TryChain::Second(fut2) => {
                    // Poll the second future
                    return unsafe { PinMut::new_unchecked(fut2) }.try_poll(cx)
                }
                TryChain::Empty => {
                    panic!("future must not be polled after it returned `Poll::Ready`");
                }
            };

            *this = TryChain::Empty; // Drop fut1
            let op = op.take().unwrap();
            match op(output, data) {
                TryChainAction::Future(fut2) => *this = TryChain::Second(fut2),
                TryChainAction::Output(output) => return Poll::Ready(output),
            }
        }
    }
}
