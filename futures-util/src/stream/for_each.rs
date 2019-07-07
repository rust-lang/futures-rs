use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`for_each`](super::StreamExt::for_each) method.
#[unsafe_project(Unpin)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ForEach<St, Fut, F> {
    #[pin]
    stream: St,
    f: F,
    #[pin]
    future: Option<Fut>,
}

impl<St, Fut, F> fmt::Debug for ForEach<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEach")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> ForEach<St, Fut, F>
where St: Stream,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()>,
{
    pub(super) fn new(stream: St, f: F) -> ForEach<St, Fut, F> {
        ForEach {
            stream,
            f,
            future: None,
        }
    }
}

impl<St: FusedStream, Fut, F> FusedFuture for ForEach<St, Fut, F> {
    fn is_terminated(&self) -> bool {
        self.future.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Future for ForEach<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = ()>,
{
    type Output = ();

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        loop {
            if let Some(future) = self.future.as_mut().as_pin_mut() {
                ready!(future.poll(cx));
            }
            self.future.set(None);

            match ready!(self.stream.as_mut().poll_next(cx)) {
                Some(e) => {
                    let future = (self.f)(e);
                    self.future.set(Some(future));
                }
                None => {
                    return Poll::Ready(());
                }
            }
        }
    }
}
