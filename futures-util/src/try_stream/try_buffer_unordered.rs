use crate::stream::{Fuse, FuturesUnordered, StreamExt};
use crate::try_future::{IntoFuture, TryFutureExt};
use crate::try_stream::IntoStream;
use futures_core::future::TryFuture;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::marker::Unpin;
use std::pin::Pin;

/// A stream returned by the
/// [`try_buffer_unordered`](super::TryStreamExt::try_buffer_unordered) method
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryBufferUnordered<St>
    where St: TryStream
{
    stream: Fuse<IntoStream<St>>,
    in_progress_queue: FuturesUnordered<IntoFuture<St::Ok>>,
    max: usize,
}

impl<St> Unpin for TryBufferUnordered<St>
    where St: TryStream + Unpin
{}

impl<St> TryBufferUnordered<St>
    where St: TryStream,
          St::Ok: TryFuture,
{
    unsafe_pinned!(stream: Fuse<IntoStream<St>>);
    unsafe_unpinned!(in_progress_queue: FuturesUnordered<IntoFuture<St::Ok>>);

    pub(super) fn new(stream: St, n: usize) -> Self {
        TryBufferUnordered {
            stream: IntoStream::new(stream).fuse(),
            in_progress_queue: FuturesUnordered::new(),
            max: n,
        }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        self.stream.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        self.stream.get_mut().get_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner().into_inner()
    }
}

impl<St> Stream for TryBufferUnordered<St>
    where St: TryStream,
          St::Ok: TryFuture<Error = St::Error>,
{
    type Item = Result<<St::Ok as TryFuture>::Ok, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<Self::Item>> {
        // First up, try to spawn off as many futures as possible by filling up
        // our slab of futures. Propagate errors from the stream immediately.
        while self.in_progress_queue.len() < self.max {
            match self.stream().poll_next(lw) {
                Poll::Ready(Some(Ok(fut))) => self.in_progress_queue().push(fut.into_future()),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Attempt to pull the next value from the in_progress_queue
        match Pin::new(self.in_progress_queue()).poll_next(lw) {
            x @ Poll::Pending | x @ Poll::Ready(Some(_)) => return x,
            Poll::Ready(None) => {}
        }

        // If more values are still coming from the stream, we're not done yet
        if self.stream.is_done() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}
