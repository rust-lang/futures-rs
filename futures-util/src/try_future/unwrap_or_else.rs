use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`unwrap_or_else`](super::TryFutureExt::unwrap_or_else)
/// method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct UnwrapOrElse<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> UnwrapOrElse<Fut, F> {
    pub(super) fn new(future: Fut, f: F) -> UnwrapOrElse<Fut, F> {
        UnwrapOrElse { future, f: Some(f) }
    }
}

impl<Fut, F> FusedFuture for UnwrapOrElse<Fut, F> {
    fn is_terminated(&self) -> bool {
        self.f.is_none()
    }
}

impl<Fut, F> Future for UnwrapOrElse<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> Fut::Ok,
{
    type Output = Fut::Ok;

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
                    .expect("UnwrapOrElse already returned `Poll::Ready` before");
                result.unwrap_or_else(op)
            })
    }
}
