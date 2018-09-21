use core::marker::Unpin;
use core::pin::PinMut;
use futures_core::future::{Future, TryFuture};
use futures_core::stream::TryStream;
use futures_core::task::{self, Poll};
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

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        loop {
            if let Some(future) = self.future().as_pin_mut() {
                try_ready!(future.try_poll(cx));
            }
            PinMut::set(self.future(), None);

            match ready!(self.stream().try_poll_next(cx)) {
                Some(Ok(e)) => {
                    let future = (self.f())(e);
                    PinMut::set(self.future(), Some(future));
                }
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(())),
            }
        }
    }
}
