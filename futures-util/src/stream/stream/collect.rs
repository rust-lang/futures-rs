use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::iteration;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`collect`](super::StreamExt::collect) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Collect<St, C> {
    stream: St,
    collection: C,
    yield_after: iteration::Limit,
}

impl<St: Unpin, C> Unpin for Collect<St, C> {}

impl<St: Stream, C: Default> Collect<St, C> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(collection: C);
    unsafe_unpinned!(yield_after: iteration::Limit);

    fn finish(mut self: Pin<&mut Self>) -> C {
        mem::replace(self.as_mut().collection(), Default::default())
    }

    future_method_yield_after_every!();

    pub(super) fn new(stream: St) -> Collect<St, C> {
        Collect {
            stream,
            collection: Default::default(),
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

impl<St, C> FusedFuture for Collect<St, C>
where St: FusedStream,
      C: Default + Extend<St:: Item>
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for Collect<St, C>
where St: Stream,
      C: Default + Extend<St:: Item>
{
    type Output = C;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<C> {
        poll_loop! { self.as_mut().yield_after(), cx,
            match ready!(self.as_mut().stream().poll_next(cx)) {
                Some(e) => self.as_mut().collection().extend(Some(e)),
                None => return Poll::Ready(self.as_mut().finish()),
            }
        }
    }
}
