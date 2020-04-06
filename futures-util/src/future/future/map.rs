use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

use crate::fns::FnOnce1;

/// Future for the [`map`](super::FutureExt::map) method.
#[pin_project]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Map<Fut, F> {
    #[pin]
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> Map<Fut, F> {
    /// Creates a new Map.
    pub(crate) fn new(future: Fut, f: F) -> Map<Fut, F> {
        Map { future, f: Some(f) }
    }
}

impl<Fut, F, T> FusedFuture for Map<Fut, F>
    where Fut: Future,
          F: FnOnce1<Fut::Output, Output=T>,
{
    fn is_terminated(&self) -> bool { self.f.is_none() }
}

impl<Fut, F, T> Future for Map<Fut, F>
    where Fut: Future,
          F: FnOnce1<Fut::Output, Output=T>,
{
    type Output = T;

    #[project]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        #[project]
        let Map { future, f } = self.project();
        let output = ready!(future.poll(cx));
        let f = f.take()
            .expect("Map must not be polled after it returned `Poll::Ready`");

        Poll::Ready(f.call_once(output))
    }
}
