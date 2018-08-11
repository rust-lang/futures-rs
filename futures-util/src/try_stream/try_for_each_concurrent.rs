use crate::stream::{FuturesUnordered, StreamExt};
use core::marker::Unpin;
use core::mem::PinMut;
use core::num::NonZeroUsize;
use futures_core::future::Future;
use futures_core::stream::TryStream;
use futures_core::task::{self, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which executes a unit closure over each item on a
/// stream concurrently.
///
/// This structure is returned by the
/// [`TryStreamExt::try_for_each_concurrent`](super::TryStreamExt::try_for_each_concurrent)
/// method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TryForEachConcurrent<St, Fut, F> {
    stream: Option<St>,
    f: F,
    futures: FuturesUnordered<Fut>,
    limit: Option<NonZeroUsize>,
}

impl<St, Fut, F> Unpin for TryForEachConcurrent<St, Fut, F>
where St: Unpin,
      Fut: Unpin,
{}

impl<St, Fut, F> TryForEachConcurrent<St, Fut, F>
where St: TryStream,
      F: FnMut(St::Ok) -> Fut,
      Fut: Future<Output = Result<(), St::Error>>,
{
    unsafe_pinned!(stream: Option<St>);
    unsafe_unpinned!(f: F);
    unsafe_unpinned!(futures: FuturesUnordered<Fut>);
    unsafe_unpinned!(limit: Option<NonZeroUsize>);

    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> TryForEachConcurrent<St, Fut, F> {
        TryForEachConcurrent {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
        }
    }
}

impl<St, Fut, F> Future for TryForEachConcurrent<St, Fut, F>
    where St: TryStream,
          F: FnMut(St::Ok) -> Fut,
          Fut: Future<Output = Result<(), St::Error>>,
{
    type Output = Result<(), St::Error>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        loop {
            let mut made_progress_this_iter = false;

            // Try and pull an item from the stream
            let current_len = self.futures().len();
            // Check if we've already created a number of futures greater than `limit`
            if self.limit().map(|limit| limit.get() > current_len).unwrap_or(true) {
                let mut stream_completed = false;
                let elem = if let Some(stream) = self.stream().as_pin_mut() {
                    match stream.try_poll_next(cx)? {
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
                    PinMut::set(self.stream(), None);
                }
                if let Some(elem) = elem {
                    let next_future = (self.f())(elem);
                    self.futures().push(next_future);
                }
            }

            match self.futures().poll_next_unpin(cx)? {
                Poll::Ready(Some(())) => made_progress_this_iter = true,
                Poll::Ready(None) => {
                    if self.stream().is_none() {
                        return Poll::Ready(Ok(()))
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
