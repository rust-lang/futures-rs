use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`err_into`](super::TryFutureExt::err_into) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ErrInto<Fut, E> {
    #[pin]
    future: Fut,
    _marker: PhantomData<E>,
}

impl<Fut, E> ErrInto<Fut, E> {
    pub(super) fn new(future: Fut) -> ErrInto<Fut, E> {
        ErrInto {
            future,
            _marker: PhantomData,
        }
    }
}

impl<Fut: FusedFuture, E> FusedFuture for ErrInto<Fut, E> {
    fn is_terminated(&self) -> bool { self.future.is_terminated() }
}

impl<Fut, E> Future for ErrInto<Fut, E>
    where Fut: TryFuture,
          Fut::Error: Into<E>,
{
    type Output = Result<Fut::Ok, E>;

    #[pin_project(self)]
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.future.try_poll(cx)
            .map(|res| res.map_err(Into::into))
    }
}
