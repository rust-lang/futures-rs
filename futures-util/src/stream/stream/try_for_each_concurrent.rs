use crate::stream::{FuturesUnordered, StreamExt};
use core::fmt;
use core::num::NonZeroUsize;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the
    /// [`try_for_each_concurrent`](super::TryStreamExt::try_for_each_concurrent)
    /// method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryForEachConcurrent<St, Fut, F> {
        #[pin]
        stream: Option<St>,
        f: F,
        futures: FuturesUnordered<Fut>,
        limit: Option<NonZeroUsize>,
    }
}

impl<St, Fut, F> fmt::Debug for TryForEachConcurrent<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryForEachConcurrent")
            .field("stream", &self.stream)
            .field("futures", &self.futures)
            .field("limit", &self.limit)
            .finish()
    }
}

impl<St, Fut, F, E> FusedFuture for TryForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_none() && self.futures.is_empty()
    }
}

impl<St, Fut, F, E> TryForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> Self {
        Self {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
        }
    }
}

impl<St, Fut, F, E> Future for TryForEachConcurrent<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            let mut made_progress_this_iter = false;

            // Check if we've already created a number of futures greater than `limit`
            if this.limit.map(|limit| limit.get() > this.futures.len()).unwrap_or(true) {
                let poll_res = match this.stream.as_mut().as_pin_mut() {
                    Some(stream) => stream.poll_next(cx),
                    None => Poll::Ready(None),
                };

                let elem = match poll_res {
                    Poll::Ready(Some(elem)) => {
                        made_progress_this_iter = true;
                        Some(elem)
                    }
                    Poll::Ready(None) => {
                        this.stream.set(None);
                        None
                    }
                    Poll::Pending => None,
                };

                if let Some(elem) = elem {
                    this.futures.push((this.f)(elem));
                }
            }

            match this.futures.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(()))) => made_progress_this_iter = true,
                Poll::Ready(None) => {
                    if this.stream.is_none() {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Pending => {}
                Poll::Ready(Some(Err(e))) => {
                    // Empty the stream and futures so that we know
                    // the future has completed.
                    this.stream.set(None);
                    drop(core::mem::take(this.futures));
                    return Poll::Ready(Err(e));
                }
            }

            if !made_progress_this_iter {
                return Poll::Pending;
            }
        }
    }
}
