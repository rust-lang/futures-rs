use crate::stream::{FuturesUnordered, StreamExt};
use core::fmt;
use core::mem;
use core::num::{NonZeroU32, NonZeroUsize};
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the
/// [`try_for_each_concurrent`](super::TryStreamExt::try_for_each_concurrent)
/// method.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryForEachConcurrent<St, Fut, F> {
    stream: Option<St>,
    f: F,
    futures: FuturesUnordered<Fut>,
    limit: Option<NonZeroUsize>,
    yield_after: NonZeroU32,
}

impl<St, Fut, F> Unpin for TryForEachConcurrent<St, Fut, F>
where St: Unpin,
      Fut: Unpin,
{}

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
            .field("yield_after", &self.yield_after)
            .finish()
    }
}

impl<St, Fut, F> FusedFuture for TryForEachConcurrent<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: Future<Output = Result<(), St::Error>>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_none() && self.futures.is_empty()
    }
}

impl<St, Fut, F> TryForEachConcurrent<St, Fut, F>
where St: TryStream,
      F: FnMut(St::Ok) -> Fut,
      Fut: Future<Output = Result<(), St::Error>>,
{
    unsafe_pinned!(stream: Option<St>);
    unsafe_unpinned!(f: F);
    unsafe_unpinned!(futures: FuturesUnordered<Fut>);
    unsafe_unpinned!(limit: Option<NonZeroUsize>);
    unsafe_unpinned!(yield_after: NonZeroU32);

    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> TryForEachConcurrent<St, Fut, F> {
        TryForEachConcurrent {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
            yield_after: crate::DEFAULT_YIELD_AFTER_LIMIT,
        }
    }
}

impl<St, Fut, F> Future for TryForEachConcurrent<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: Future<Output = Result<(), St::Error>>,
{
    type Output = Result<(), St::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        poll_loop! { self.yield_after, cx, {
            let mut made_progress_this_iter = false;

            // Try and pull an item from the stream
            let current_len = self.futures.len();
            // Check if we've already created a number of futures greater than `limit`
            if self.limit.map(|limit| limit.get() > current_len).unwrap_or(true) {
                let poll_res = match self.as_mut().stream().as_pin_mut() {
                    Some(stream) => stream.try_poll_next(cx),
                    None => Poll::Ready(None),
                };

                let elem = match poll_res {
                    Poll::Ready(Some(Ok(elem))) => {
                        made_progress_this_iter = true;
                        Some(elem)
                    },
                    Poll::Ready(None) => {
                        self.as_mut().stream().set(None);
                        None
                    }
                    Poll::Pending => None,
                    Poll::Ready(Some(Err(e))) => {
                        // Empty the stream and futures so that we know
                        // the future has completed.
                        self.as_mut().stream().set(None);
                        drop(mem::replace(self.as_mut().futures(), FuturesUnordered::new()));
                        return Poll::Ready(Err(e));
                    }
                };

                if let Some(elem) = elem {
                    let next_future = (self.as_mut().f())(elem);
                    self.as_mut().futures().push(next_future);
                }
            }

            match self.as_mut().futures().poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(()))) => made_progress_this_iter = true,
                Poll::Ready(None) => {
                    if self.stream.is_none() {
                        return Poll::Ready(Ok(()))
                    }
                },
                Poll::Pending => {}
                Poll::Ready(Some(Err(e))) => {
                    // Empty the stream and futures so that we know
                    // the future has completed.
                    self.as_mut().stream().set(None);
                    drop(mem::replace(self.as_mut().futures(), FuturesUnordered::new()));
                    return Poll::Ready(Err(e));
                }
            }

            if !made_progress_this_iter {
                return Poll::Pending;
            }
        }}
    }
}
