use crate::stream::{Fuse, FusedStream, FuturesOrdered, StreamExt};
use core::fmt;
use core::num::NonZeroUsize;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`buffered`](super::StreamExt::buffered) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Buffered<St>
    where
        St: Stream,
        St::Item: Future,
    {
        #[pin]
        stream: Fuse<St>,
        in_progress_queue: FuturesOrdered<St::Item>,
        max: Option<NonZeroUsize>,
    }
}

impl<St> fmt::Debug for Buffered<St>
where
    St: Stream + fmt::Debug,
    St::Item: Future,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("stream", &self.stream)
            .field("in_progress_queue", &self.in_progress_queue)
            .field("max", &self.max)
            .finish()
    }
}

impl<St> Buffered<St>
where
    St: Stream,
    St::Item: Future,
{
    pub(super) fn new(stream: St, n: Option<usize>) -> Self {
        Self {
            stream: super::Fuse::new(stream),
            in_progress_queue: FuturesOrdered::new(),
            max: n.and_then(NonZeroUsize::new),
        }
    }

    delegate_access_inner!(stream, St, (.));

    /// Fill the buffer as much as is allowed by polling the underlying `Stream`.
    ///
    /// Returns `false` if there are no more futures and the `Stream::is_terminated()`.
    /// Otherwise there may be more futures, but the buffer is out of room.
    fn poll_fill(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<bool> {
        let mut this = self.project();

        while this.max.map(|max| this.in_progress_queue.len() < max.get()).unwrap_or(true) {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(fut) => this.in_progress_queue.push_back(fut),
                None => break,
            }
        }

        Poll::Ready(!this.stream.is_done())
    }

    /// Poll the buffered `Stream`, allowing it to progress as long as there is
    /// room in the buffer.
    ///
    /// This will also poll any futures produced by the stream, but only polls
    /// the underlying `Stream` as long as the buffer can hold more entries.
    /// `Stream::poll_next` should be used to progress to completion.
    ///
    /// When `Poll::Ready` is returned, the underlying stream has been
    /// exhausted, and all of its futures have been run to completion.
    pub fn poll_stream(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let stream_res = self.as_mut().poll_fill(cx);

        let this = self.project();
        let queue_res = Pin::new(&mut *this.in_progress_queue).poll_all(cx);

        ready!(stream_res);
        queue_res
    }
}

impl<St> Stream for Buffered<St>
where
    St: Stream,
    St::Item: Future,
{
    type Item = <St::Item as Future>::Output;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // First up, try to spawn off as many futures as possible by filling up
        // our queue of futures.
        let stream_res = self.as_mut().poll_fill(cx);

        let this = self.project();

        // Attempt to pull the next value from the in_progress_queue
        let res = this.in_progress_queue.poll_next_unpin(cx);
        if let Some(val) = ready!(res) {
            return Poll::Ready(Some(val));
        }

        // If more values are still coming from the stream, we're not done yet
        match stream_res {
            Poll::Pending => Poll::Pending,
            Poll::Ready(false) => Poll::Ready(None),
            Poll::Ready(true) => panic!("buffer is full, but we have no values???"),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let queue_len = self.in_progress_queue.len();
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(queue_len);
        let upper = match upper {
            Some(x) => x.checked_add(queue_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St> FusedStream for Buffered<St>
where
    St: Stream,
    St::Item: Future,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_done() && self.in_progress_queue.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Buffered<S>
where
    S: Stream + Sink<Item>,
    S::Item: Future,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
