use core::fmt;
use core::pin::Pin;
use futures_core::future::{Future, TryFuture};
use futures_core::iteration;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`try_for_each`](super::TryStreamExt::try_for_each) method.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryForEach<St, Fut, F> {
    stream: St,
    f: F,
    future: Option<Fut>,
    yield_after: iteration::Limit,
}

impl<St: Unpin, Fut: Unpin, F> Unpin for TryForEach<St, Fut, F> {}

impl<St, Fut, F> fmt::Debug for TryForEach<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryForEach")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .field("yield_after", &self.yield_after)
            .finish()
    }
}

impl<St, Fut, F> TryForEach<St, Fut, F>
where St: TryStream,
      F: FnMut(St::Ok) -> Fut,
      Fut: TryFuture<Ok = (), Error = St::Error>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(future: Option<Fut>);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn split_borrows(
        self: Pin<&mut Self>,
    ) -> (
        Pin<&mut St>,
        &mut F,
        Pin<&mut Option<Fut>>,
        &mut iteration::Limit,
    ) {
        unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.stream),
                &mut this.f,
                Pin::new_unchecked(&mut this.future),
                &mut this.yield_after,
            )
        }
    }

    future_method_yield_after_every! {
        #[pollee = "the underlying stream and, if pending, a future returned by
            the processing closure,"]
        #[why_busy = "the underlying stream consecutively yields `Ok` items and
            the processing futures immediately resolve with `Ok`,"]
    }

    pub(super) fn new(stream: St, f: F) -> TryForEach<St, Fut, F> {
        TryForEach {
            stream,
            f,
            future: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

impl<St, Fut, F> Future for TryForEach<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: TryFuture<Ok = (), Error = St::Error>,
{
    type Output = Result<(), St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut stream, op, mut future, yield_after) = self.split_borrows();
        poll_loop! { yield_after, cx, {
            if let Some(future) = future.as_mut().as_pin_mut() {
                ready!(future.try_poll(cx))?;
            }
            future.as_mut().set(None);

            match ready!(stream.as_mut().try_poll_next(cx)?) {
                Some(e) => {
                    let fut = op(e);
                    future.as_mut().set(Some(fut));
                }
                None => return Poll::Ready(Ok(())),
            }
        }}
    }
}
