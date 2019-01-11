use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::stream::TryStream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which attempts to execute an async closure over each
/// future in a stream.
///
/// This future is returned by the `TryStream::try_for_each` method.
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

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        loop {
            if let Some(future) = self.as_mut().future().as_pin_mut() {
                try_ready!(future.try_poll(lw));
            }
            Pin::set(&mut self.as_mut().future(), None);

            match ready!(self.as_mut().stream().try_poll_next(lw)) {
                Some(Ok(e)) => {
                    let future = (self.as_mut().f())(e);
                    Pin::set(&mut self.as_mut().future(), Some(future));
                }
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(())),
            }
        }
    }
}
