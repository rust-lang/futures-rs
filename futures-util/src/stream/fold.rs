use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`fold`](super::StreamExt::fold) method.
#[unsafe_project(Unpin)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Fold<St, Fut, T, F> {
    #[pin]
    stream: St,
    f: F,
    accum: Option<T>,
    #[pin]
    future: Option<Fut>,
}

impl<St, Fut, T, F> fmt::Debug for Fold<St, Fut, T, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fold")
            .field("stream", &self.stream)
            .field("accum", &self.accum)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, T, F> Fold<St, Fut, T, F>
where St: Stream,
      F: FnMut(T, St::Item) -> Fut,
      Fut: Future<Output = T>,
{
    pub(super) fn new(stream: St, f: F, t: T) -> Fold<St, Fut, T, F> {
        Fold {
            stream,
            f,
            accum: Some(t),
            future: None,
        }
    }
}

impl<St, Fut, T, F> FusedFuture for Fold<St, Fut, T, F> {
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.future.is_none()
    }
}

impl<St, Fut, T, F> Future for Fold<St, Fut, T, F>
    where St: Stream,
          F: FnMut(T, St::Item) -> Fut,
          Fut: Future<Output = T>,
{
    type Output = T;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        loop {
            // we're currently processing a future to produce a new accum value
            if self.accum.is_none() {
                let accum = ready!(self.future.as_mut().as_pin_mut().unwrap().poll(cx));
                *self.accum = Some(accum);
                self.future.set(None);
            }

            let item = ready!(self.stream.as_mut().poll_next(cx));
            let accum = self.accum.take().expect("Fold polled after completion");

            if let Some(e) = item {
                let future = (self.f)(accum, e);
                self.future.set(Some(future));
            } else {
                return Poll::Ready(accum)
            }
        }
    }
}
