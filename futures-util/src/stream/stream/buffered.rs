use crate::stream::{Fuse, FusedStream, FuturesOrdered, StreamExt};
use alloc::collections::VecDeque;
use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`buffered`](super::StreamExt::buffered) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct Buffered<St, F>
    where
        St: Stream<Item = F>,
        F: Future,
    {
        #[pin]
        stream: Fuse<St>,
        in_progress_queue: FuturesOrdered<St::Item>,
        ready_queue: VecDeque<F::Output>,
        max: usize,
    }
}

impl<St, F> fmt::Debug for Buffered<St, F>
where
    St: Stream<Item = F> + fmt::Debug,
    F: Future,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffered")
            .field("stream", &self.stream)
            .field("in_progress_queue", &self.in_progress_queue)
            .field("max", &self.max)
            .finish()
    }
}

impl<St, F> Buffered<St, F>
where
    St: Stream<Item = F>,
    F: Future,
{
    pub(super) fn new(stream: St, n: usize) -> Self {
        Self {
            stream: super::Fuse::new(stream),
            in_progress_queue: FuturesOrdered::new(),
            ready_queue: VecDeque::new(),
            max: n,
        }
    }

    delegate_access_inner!(stream, St, (.));
}

impl<St, F> Stream for Buffered<St, F>
where
    St: Stream<Item = F>,
    F: Future,
{
    type Item = <St::Item as Future>::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First up, try to spawn off as many futures as possible by filling up
        // our queue of futures.
        while this.in_progress_queue.len() < *this.max {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(fut)) => this.in_progress_queue.push_back(fut),
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        // Try to poll all ready futures in the in_progress_queue.
        loop {
            match this.in_progress_queue.poll_next_unpin(cx) {
                Poll::Ready(Some(output)) => {
                    this.ready_queue.push_back(output);
                }
                Poll::Ready(None) => break,
                Poll::Pending => break,
            }
        }

        if let Some(output) = this.ready_queue.pop_front() {
            // If we have any ready outputs, return the first one.
            Poll::Ready(Some(output))
        } else if this.stream.is_done() && this.in_progress_queue.is_empty() {
            Poll::Ready(None)
        } else {
            Poll::Pending
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

impl<St, F> FusedStream for Buffered<St, F>
where
    St: Stream<Item = F>,
    F: Future,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_done() && self.in_progress_queue.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, F, Item> Sink<Item> for Buffered<S, F>
where
    S: Stream<Item = F> + Sink<Item>,
    F: Future,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
