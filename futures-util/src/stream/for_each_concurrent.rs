use crate::stream::{FuturesUnordered, StreamExt};
use core::marker::Unpin;
use core::pin::Pin;
use core::num::NonZeroUsize;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which executes a unit closure over each item on a
/// stream concurrently.
///
/// This structure is returned by the
/// [`StreamExt::for_each_concurrent`](super::StreamExt::for_each_concurrent)
/// method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEachConcurrent<St, Fut, F> {
    stream: Option<St>,
    f: F,
    futures: FuturesUnordered<Fut>,
    limit: Option<NonZeroUsize>,
}

impl<St, Fut, F> Unpin for ForEachConcurrent<St, Fut, F>
where St: Unpin,
      Fut: Unpin,
{}

impl<St, Fut, F> ForEachConcurrent<St, Fut, F>
where St: Stream,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()>,
{
    unsafe_pinned!(stream: Option<St>);
    unsafe_unpinned!(f: F);
    unsafe_unpinned!(futures: FuturesUnordered<Fut>);
    unsafe_unpinned!(limit: Option<NonZeroUsize>);

    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> ForEachConcurrent<St, Fut, F> {
        ForEachConcurrent {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
        }
    }
}

impl<St, Fut, F> Future for ForEachConcurrent<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<()> {
        loop {
            let mut made_progress_this_iter = false;

            // Try and pull an item from the stream
            let current_len = self.futures().len();
            // Check if we've already created a number of futures greater than `limit`
            if self.limit().map(|limit| limit.get() > current_len).unwrap_or(true) {
                let mut stream_completed = false;
                let elem = if let Some(stream) = self.stream().as_pin_mut() {
                    match stream.poll_next(lw) {
                        Poll::Ready(Some(elem)) => {
                            made_progress_this_iter = true;
                            Some(elem)
                        },
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
                    Pin::set(self.stream(), None);
                }
                if let Some(elem) = elem {
                    let next_future = (self.f())(elem);
                    self.futures().push(next_future);
                }
            }

            match self.futures().poll_next_unpin(lw) {
                Poll::Ready(Some(())) => made_progress_this_iter = true,
                Poll::Ready(None) => {
                    if self.stream().is_none() {
                        return Poll::Ready(())
                    }
                },
                Poll::Pending => {}
            }

            if !made_progress_this_iter {
                return Poll::Pending;
            }
        }
    }
}
