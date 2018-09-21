use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::marker::Unpin;
use std::mem;
use std::pin::PinMut;
use std::prelude::v1::*;

/// A future which attempts to collect all of the values of a stream.
///
/// This future is created by the `Stream::try_collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryCollect<St, C> where St: TryStream {
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

    fn finish(mut self: PinMut<Self>) -> C {
        mem::replace(self.items(), Default::default())
    }
}

impl<St: Unpin + TryStream, C> Unpin for TryCollect<St, C> {}

impl<St, C> Future for TryCollect<St, C>
    where St: TryStream, C: Default + Extend<St::Ok>
{
    type Output = Result<C, St::Error>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream().try_poll_next(cx)) {
                Some(Ok(x)) => self.items().extend(Some(x)),
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(self.finish())),
            }
        }
    }
}
