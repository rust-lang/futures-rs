use crate::stream::{FuturesUnordered, StreamExt};
use core::fmt;
use core::num::NonZeroUsize;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`for_each_concurrent`](super::StreamExt::for_each_concurrent)
    /// method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ForEachConcurrent<St, Fut, F> {
        #[pin]
        stream: Option<St>,
        f: F,
        futures: FuturesUnordered<Fut>,
        limit: Option<NonZeroUsize>,
        completed: usize,
    }
}

impl<St, Fut, F> fmt::Debug for ForEachConcurrent<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEachConcurrent")
            .field("stream", &self.stream)
            .field("futures", &self.futures)
            .field("limit", &self.limit)
            .field("completed", &self.completed)
            .finish()
    }
}

impl<St, Fut, F> ForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> Self {
        Self {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
            completed: 0,
        }
    }
}

impl<St, Fut, F> FusedFuture for ForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_none() && self.futures.is_empty()
    }
}

impl<St, Fut, F> Future for ForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<usize> {
        let mut this = self.project();
        loop {
            let mut made_progress_this_iter = false;

            // Check if we've already created a number of futures greater than `limit`
            if this.limit.map(|limit| limit.get() > this.futures.len()).unwrap_or(true) {
                let mut stream_completed = false;
                let elem = if let Some(stream) = this.stream.as_mut().as_pin_mut() {
                    match stream.poll_next(cx) {
                        Poll::Ready(Some(elem)) => {
                            made_progress_this_iter = true;
                            Some(elem)
                        }
                        Poll::Ready(None) => {
                            stream_completed = true;
                            None
                        }
                        Poll::Pending => None,
                    }
                } else {
                    None
                };
                if stream_completed {
                    this.stream.set(None);
                }
                if let Some(elem) = elem {
                    this.futures.push((this.f)(elem));
                }
            }

            match this.futures.poll_next_unpin(cx) {
                Poll::Ready(Some(())) => {
                    // On overflow the returned count serves as a lower bound
                    // for the actual number of completed futures.
                    *this.completed = this.completed.saturating_add(1);
                    made_progress_this_iter = true;
                }
                Poll::Ready(None) => {
                    debug_assert!(this.futures.is_empty());
                    if this.stream.is_none() {
                        return Poll::Ready(*this.completed);
                    }
                }
                Poll::Pending => {
                    debug_assert!(!this.futures.is_empty());
                }
            }

            if !made_progress_this_iter {
                return Poll::Pending;
            }
        }
    }
}
