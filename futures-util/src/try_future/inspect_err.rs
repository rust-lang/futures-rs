use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`inspect_err`](super::TryFutureExt::inspect_err) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct InspectErr<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> InspectErr<Fut, F>
where
    Fut: TryFuture,
    F: FnOnce(&Fut::Error),
{
    pub(super) fn new(future: Fut, f: F) -> Self {
        Self { future, f: Some(f) }
    }
}

impl<Fut: FusedFuture, F> FusedFuture for InspectErr<Fut, F> {
    fn is_terminated(&self) -> bool {
        self.future.is_terminated()
    }
}

impl<Fut, F> Future for InspectErr<Fut, F>
where
    Fut: TryFuture,
    F: FnOnce(&Fut::Error),
{
    type Output = Result<Fut::Ok, Fut::Error>;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let e = ready!(self.future.as_mut().try_poll(cx));
        if let Err(e) = &e {
            self.f.take().expect("cannot poll InspectErr twice")(e);
        }
        Poll::Ready(e)
    }
}
