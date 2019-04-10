use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`try_for_each`](super::TryStreamExt::try_for_each) method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryForEach<St, Fut, F> {
    stream: St,
    f: F,
    future: Option<Fut>,
}

impl<St: Unpin, Fut: Unpin, F> Unpin for TryForEach<St, Fut, F> {}

impl<St, Fut, F> TryForEach<St, Fut, F>
where St: TryStream,
      F: FnMut(St::Ok) -> Fut,
      Fut: TryFuture<Ok = (), Error = St::Error>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(future: Option<Fut>);

    pub(super) fn new(stream: St, f: F) -> TryForEach<St, Fut, F> {
        TryForEach {
            stream,
            f,
            future: None,
        }
    }
}

impl<St, Fut, F> Future for TryForEach<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Ok = (), Error = St::Error>,
{
    type Output = Result<(), St::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            if let Some(future) = self.as_mut().future().as_pin_mut() {
                try_ready!(future.try_poll(cx));
            }
            self.as_mut().future().set(None);

            match ready!(self.as_mut().stream().try_poll_next(cx)) {
                Some(Ok(e)) => {
                    let future = (self.as_mut().f())(e);
                    self.as_mut().future().set(Some(future));
                }
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(())),
            }
        }
    }
}
