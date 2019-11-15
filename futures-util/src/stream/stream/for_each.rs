use core::fmt;
use core::num::NonZeroU32;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`for_each`](super::StreamExt::for_each) method.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ForEach<St, Fut, F> {
    stream: St,
    f: F,
    future: Option<Fut>,
    yield_after: NonZeroU32,
}

impl<St, Fut, F> Unpin for ForEach<St, Fut, F>
where
    St: Unpin,
    Fut: Unpin,
{}

impl<St, Fut, F> fmt::Debug for ForEach<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEach")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .field("yield_after", &self.yield_after)
            .finish()
    }
}

impl<St, Fut, F> ForEach<St, Fut, F>
where St: Stream,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(future: Option<Fut>);
    unsafe_unpinned!(yield_after: NonZeroU32);

    future_method_yield_after_every! {
        #[doc = "the underlying stream and, if pending, a future returned by
            the processing closure,"]
        #[doc = "the underlying stream consecutively yields items and
            the processing futures immediately resolve as ready,"]
    }

    pub(super) fn new(stream: St, f: F) -> ForEach<St, Fut, F> {
        ForEach {
            stream,
            f,
            future: None,
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

impl<St, Fut, F> FusedFuture for ForEach<St, Fut, F>
    where St: FusedStream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = ()>,
{
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

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        poll_loop! { self.yield_after, cx, {
            if let Some(future) = self.as_mut().future().as_pin_mut() {
                ready!(future.poll(cx));
            }
            self.as_mut().future().set(None);

            match ready!(self.as_mut().stream().poll_next(cx)) {
                Some(e) => {
                    let future = (self.as_mut().f())(e);
                    self.as_mut().future().set(Some(future));
                }
                None => {
                    return Poll::Ready(());
                }
            }
        }}
    }
}
