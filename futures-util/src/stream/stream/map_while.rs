use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`map_while`](super::StreamExt::map_while) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct MapWhile<St: Stream, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending_fut: Option<Fut>,
        done_mapping: bool,
    }
}

impl<St, Fut, F> fmt::Debug for MapWhile<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapWhile")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("done_mapping", &self.done_mapping)
            .finish()
    }
}

impl<St, Fut, F, T> MapWhile<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = Option<T>>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, pending_fut: None, done_mapping: false }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F, T> Stream for MapWhile<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = Option<T>>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        if self.done_mapping {
            return Poll::Ready(None);
        }

        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let mapped = ready!(fut.poll(cx));
                this.pending_fut.set(None);

                if mapped.is_none() {
                    *this.done_mapping = true;
                }

                break mapped;
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                this.pending_fut.set(Some((this.f)(item)));
            } else {
                break None;
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.done_mapping {
            return (0, Some(0));
        }

        let (_, upper) = self.stream.size_hint();
        (0, upper) // can't know a lower bound, due to the predicate
    }
}

impl<St, Fut, F, T> FusedStream for MapWhile<St, Fut, F>
where
    St: FusedStream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = Option<T>>,
{
    fn is_terminated(&self) -> bool {
        self.done_mapping || self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item> Sink<Item> for MapWhile<S, Fut, F>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
