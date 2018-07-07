use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use std::marker::Unpin;
use std::mem::{self, PinMut};
use std::prelude::v1::*;

/// A future which attempts to collect all of the values of a stream.
///
/// This future is created by the `Stream::try_collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryCollect<S, C> where S: TryStream {
    stream: S,
    items: C,
}

pub fn new<S, C>(s: S) -> TryCollect<S, C>
    where S: TryStream, C: Default
{
    TryCollect {
        stream: s,
        items: Default::default(),
    }
}

impl<S: TryStream, C: Default> TryCollect<S, C> {
    fn finish(mut self: PinMut<Self>) -> C {
        mem::replace(self.items(), Default::default())
    }

    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(items -> C);
}

impl<S: Unpin + TryStream, C> Unpin for TryCollect<S, C> {}

impl<S, C> Future for TryCollect<S, C>
    where S: TryStream, C: Default + Extend<S::TryItem>
{
    type Output = Result<C, S::TryError>;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream().try_poll_next(cx)) {
                Some(Ok(x)) => self.items().extend(Some(x)),
                Some(Err(e)) => return Poll::Ready(Err(e)),
                None => return Poll::Ready(Ok(self.finish())),
            }
        }
    }
}
