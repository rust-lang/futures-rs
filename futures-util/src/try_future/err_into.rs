use core::marker::PhantomData;
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};

/// Future for the `err_into` combinator, changing the error type of a future.
///
/// This is created by the `Future::err_into` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ErrInto<Fut, E> {
    future: Fut,
    _marker: PhantomData<E>,
}

impl<Fut, E> ErrInto<Fut, E> {
    unsafe_pinned!(future: Fut);

    pub(super) fn new(future: Fut) -> ErrInto<Fut, E> {
        ErrInto {
            future,
            _marker: PhantomData,
        }
    }
}

impl<Fut, E> Future for ErrInto<Fut, E>
    where Fut: TryFuture,
          Fut::Error: Into<E>,
{
    type Output = Result<Fut::Ok, E>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        match self.future().try_poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(output) => {
                Poll::Ready(output.map_err(Into::into))
            }
        }
    }
}
