use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`map_err`](super::TryFutureExt::map_err) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct MapErr<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> MapErr<Fut, F> {
    pub(super) fn new(future: Fut, f: F) -> MapErr<Fut, F> {
        MapErr { future, f: Some(f) }
    }
}

impl<Fut, F> FusedFuture for MapErr<Fut, F> {
    fn is_terminated(&self) -> bool { self.f.is_none() }
}

impl<Fut, F, E> Future for MapErr<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> E,
{
    type Output = Result<Fut::Ok, E>;

    #[pin_project(self)]
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.future
            .as_mut()
            .try_poll(cx)
            .map(|result| {
                let f = self.f.take()
                    .expect("MapErr must not be polled after it returned `Poll::Ready`");
                result.map_err(f)
            })
    }
}
