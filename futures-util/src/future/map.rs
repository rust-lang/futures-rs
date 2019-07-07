use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`map`](super::FutureExt::map) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Map<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> Map<Fut, F> {
    pub(super) fn new(future: Fut, f: F) -> Map<Fut, F> {
        Map { future, f: Some(f) }
    }
}

impl<Fut, F> FusedFuture for Map<Fut, F> {
    fn is_terminated(&self) -> bool { self.f.is_none() }
}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce(Fut::Output) -> T,
{
    type Output = T;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        self.future
            .as_mut()
            .poll(cx)
            .map(|output| {
                let f = self.f.take()
                    .expect("Map must not be polled after it returned `Poll::Ready`");
                f(output)
            })
    }
}
