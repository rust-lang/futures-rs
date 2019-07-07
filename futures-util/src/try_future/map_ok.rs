use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`map_ok`](super::TryFutureExt::map_ok) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct MapOk<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> MapOk<Fut, F> {
    pub(super) fn new(future: Fut, f: F) -> MapOk<Fut, F> {
        MapOk { future, f: Some(f) }
    }
}

impl<Fut, F> FusedFuture for MapOk<Fut, F> {
    fn is_terminated(&self) -> bool {
        self.f.is_none()
    }
}

impl<Fut, F, T> Future for MapOk<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Ok) -> T,
{
    type Output = Result<T, Fut::Error>;

    #[pin_project(self)]
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.future
            .as_mut()
            .try_poll(cx)
            .map(|result| {
                let op = self.f.take()
                    .expect("MapOk must not be polled after it returned `Poll::Ready`");
                result.map(op)
            })
    }
}
