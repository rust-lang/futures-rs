use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, project};
use crate::fns::FnMut1;

/// Stream for the [`filter`](super::StreamExt::filter) method.
#[pin_project]
#[must_use = "streams do nothing unless polled"]
pub struct Filter<St, Fut, F>
    where St: Stream,
{
    #[pin]
    stream: St,
    f: F,
    #[pin]
    pending_fut: Option<Fut>,
    pending_item: Option<St::Item>,
}

impl<St, Fut, F> fmt::Debug for Filter<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .finish()
    }
}

impl<St, Fut, F> Filter<St, Fut, F>
where St: Stream,
      F: for<'a> FnMut1<&'a St::Item, Output=Fut>,
      Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Filter<St, Fut, F> {
        Filter {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
        }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F> FusedStream for Filter<St, Fut, F>
    where St: Stream + FusedStream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.pending_fut.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for Filter<St, Fut, F>
    where St: Stream,
          F: for<'a> FnMut1<&'a St::Item, Output=Fut>,
          Fut: Future<Output = bool>,
{
    type Item = St::Item;

    #[project]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St::Item>> {
        #[project]
        let Filter { mut stream, f, mut pending_fut, pending_item } = self.project();
        Poll::Ready(loop {
            if let Some(fut) = pending_fut.as_mut().as_pin_mut() {
                let res = ready!(fut.poll(cx));
                pending_fut.set(None);
                if res {
                    break pending_item.take();
                }
                *pending_item = None;
            } else if let Some(item) = ready!(stream.as_mut().poll_next(cx)) {
                pending_fut.set(Some(f.call_mut(&item)));
                *pending_item = Some(item);
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let pending_len = if self.pending_item.is_some() { 1 } else { 0 };
        let (_, upper) = self.stream.size_hint();
        let upper = match upper {
            Some(x) => x.checked_add(pending_len),
            None => None,
        };
        (0, upper) // can't know a lower bound, due to the predicate
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for Filter<S, Fut, F>
    where S: Stream + Sink<Item>,
          F: FnMut(&S::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
