use crate::future::{Shared as SharedFuture, FutureExt};
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use std::marker::Unpin;
use std::mem::PinMut;
use super::StreamExt;
use super::into_future::StreamFuture;

/// A stream that is cloneable and can be polled in multiple threads.
/// Use `Stream::shared()` method to convert any stream into a `Shared` stream.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Shared<St: Stream + Unpin>
where
    St::Item: Clone,
{
    inner: SharedFuture<Inner<St>>,
}

impl<St: Stream + Unpin> Unpin for Shared<St>
where
    St::Item: Clone,
{}

struct Inner<St: Stream + Unpin>(StreamFuture<St>) where St::Item: Clone;

impl<St> Future for Inner<St>
where
    St: Stream + Unpin,
    St::Item: Clone,
{
    type Output = Option<(St::Item, SharedFuture<Self>)>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let (item, stream) = ready!(PinMut::new(&mut self.0).poll(cx));
        Poll::Ready(item.map(|it| (it, Inner(stream.into_future()).shared())))
    }
}

impl<St> Shared<St>
where
    St: Stream + Unpin,
    St::Item: Clone,
{
    pub(super) fn new(stream: St) -> Shared<St> {
        Shared {
            inner: Inner(stream.into_future()).shared(),
        }
    }

    /// If any clone of this `Shared` has completed execution, returns its result immediately
    /// without blocking. Otherwise, returns None without triggering the work represented by
    /// this `Shared`. This does not progress the stream to the next item.
    ///
    /// Note that if the stream has reached the end (that is `poll_next` returnd
    /// `Poll::Ready(None)`) this will also return None.
    pub fn peek(&self) -> Option<St::Item> {
        match self.inner.peek() {
            Some(Some((it, _))) => Some(it),
            _ => None
        }
    }
}

impl<St> Stream for Shared<St>
where
    St: Stream + Unpin,
    St::Item: Clone,
{
    type Item = St::Item;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<St::Item>> {
        if let Some((item, next)) = ready!(PinMut::new(&mut self.inner).poll(cx)) {
            self.inner = next;
            Poll::Ready(Some(item))
        } else {
            Poll::Ready(None)
        }
    }
}

impl<St> Clone for Shared<St>
where
    St: Stream + Unpin,
    St::Item: Clone,
{
    fn clone(&self) -> Self {
        Shared {
            inner: self.inner.clone(),
        }
    }
}

