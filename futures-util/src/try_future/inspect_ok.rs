use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`inspect_ok`](super::TryFutureExt::inspect_ok) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct InspectOk<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> InspectOk<Fut, F>
where
    Fut: TryFuture,
    F: FnOnce(&Fut::Ok),
{
    pub(super) fn new(future: Fut, f: F) -> Self {
        Self { future, f: Some(f) }
    }
}

impl<Fut: FusedFuture, F> FusedFuture for InspectOk<Fut, F> {
    fn is_terminated(&self) -> bool {
        self.future.is_terminated()
    }
}

impl<Fut, F> Future for InspectOk<Fut, F>
where
    Fut: TryFuture,
    F: FnOnce(&Fut::Ok),
{
    type Output = Result<Fut::Ok, Fut::Error>;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let e = ready!(self.future.as_mut().try_poll(cx));
        if let Ok(e) = &e {
            self.f.take().expect("cannot poll InspectOk twice")(e);
        }
        Poll::Ready(e)
    }
}
