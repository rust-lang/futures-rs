use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, TryStream};
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::mem;
use std::pin::Pin;
use std::prelude::v1::*;

/// A future which attempts to collect all of the values of a stream.
///
/// This future is created by the `Stream::try_collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryCollect<St, C> {
    stream: St,
    items: C,
}

impl<St: TryStream, C: Default> TryCollect<St, C> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(items: C);

    pub(super) fn new(s: St) -> TryCollect<St, C> {
        TryCollect {
            stream: s,
            items: Default::default(),
        }
    }

    fn finish(self: Pin<&mut Self>) -> C {
        mem::replace(self.items(), Default::default())
    }
}

impl<St: Unpin + TryStream, C> Unpin for TryCollect<St, C> {}

impl<St: FusedStream, C> FusedFuture for TryCollect<St, C> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for TryCollect<St, C>
    where St: TryStream, C: Default + Extend<St::Ok>
{
    type Output = Result<C, St::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Self::Output> {
        loop {
            match ready!(self.as_mut().stream().try_poll_next(lw)) {
                Some(Ok(x)) => self.as_mut().items().extend(Some(x)),
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(self.as_mut().finish())),
            }
        }
    }
}
