use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::marker::Unpin;
use std::mem;
use std::pin::Pin;
use std::prelude::v1::*;

/// A future which collects all of the values of a stream into a vector.
///
/// This future is created by the `Stream::collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Collect<St, C> where St: Stream {
    stream: St,
    collection: C,
}

impl<St: Unpin + Stream, C> Unpin for Collect<St, C> {}

impl<St: Stream, C: Default> Collect<St, C> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(collection: C);

    fn finish(mut self: Pin<&mut Self>) -> C {
        mem::replace(self.collection(), Default::default())
    }

    pub(super) fn new(stream: St) -> Collect<St, C> {
        Collect {
            stream,
            collection: Default::default(),
        }
    }
}

impl<St, C> Future for Collect<St, C>
where St: Stream,
      C: Default + Extend<St:: Item>
{
    type Output = C;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<C> {
        loop {
            match ready!(self.stream().poll_next(lw)) {
                Some(e) => self.collection().extend(Some(e)),
                None => return Poll::Ready(self.finish()),
            }
        }
    }
}
