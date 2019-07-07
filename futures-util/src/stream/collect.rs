use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};

/// Future for the [`collect`](super::StreamExt::collect) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Collect<St, C> {
    #[pin]
    stream: St,
    collection: C,
}

impl<St: Stream, C: Default> Collect<St, C> {
    pub(super) fn new(stream: St) -> Collect<St, C> {
        Collect {
            stream,
            collection: Default::default(),
        }
    }
}

impl<St: FusedStream, C> FusedFuture for Collect<St, C> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for Collect<St, C>
where St: Stream,
      C: Default + Extend<St:: Item>
{
    type Output = C;

    #[pin_project(self)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<C> {
        loop {
            match ready!(self.stream.as_mut().poll_next(cx)) {
                Some(e) => self.collection.extend(Some(e)),
                None => return Poll::Ready(mem::replace(self.collection, Default::default())),
            }
        }
    }
}
