use core::marker::{PhantomData, Unpin};
use core::mem::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::task::{self, Poll};
use pin_utils::unsafe_pinned;

/// Future for the [`err_into`](super::TryFutureExt::err_into) combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ErrInto<Fut, E> {
    future: Fut,
    _marker: PhantomData<E>,
}

impl<Fut: Unpin, E> Unpin for ErrInto<Fut, E> {}

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
        self.future().try_poll(cx)
            .map(|res| res.map_err(Into::into))
    }
}
