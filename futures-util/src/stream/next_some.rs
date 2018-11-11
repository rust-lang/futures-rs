use core::marker::Unpin;
use core::pin::Pin;
use futures_core::stream::{Stream, FusedStream};
use futures_core::future::{Future, FusedFuture};
use futures_core::task::{LocalWaker, Poll};
use crate::stream::StreamExt;

/// A future that resolves to the next value yielded from a [`Stream`].
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct NextSome<'a, St> {
    stream: Option<&'a mut St>,
}

impl<'a, St> NextSome<'a, St> {
    pub(super) fn new(stream: &'a mut St) -> Self {
        NextSome { stream: Some(stream) }
    }
}

impl<'a, St: FusedStream> FusedFuture for NextSome<'a, St> {
    fn is_terminated(&self) -> bool {
        self.stream.as_ref().map(|s| s.is_terminated()).unwrap_or(true)
    }
}

impl<'a, St: Stream + Unpin> Future for NextSome<'a, St> {
    type Output = St::Item;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let stream = self.stream.as_mut().expect("NextSome polled after terminated");

        if let Some(item) = ready!(stream.poll_next_unpin(lw)) {
            Poll::Ready(item)
        } else {
            self.stream = None;
            lw.wake();
            Poll::Pending
        }
    }
}
