use core::fmt;
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`try_map_while`](super::TryStreamExt::try_map_while)
    /// method.
    #[must_use = "streams do nothing unless polled"]
    pub struct TryMapWhile<St, Fut, F>
    where
        St: TryStream,
    {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        pending_fut: Option<Fut>,
        done_mapping: bool,
    }
}

impl<St, Fut, F> fmt::Debug for TryMapWhile<St, Fut, F>
where
    St: TryStream + fmt::Debug,
    St::Ok: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryMapWhile")
            .field("stream", &self.stream)
            .field("pending_fut", &self.pending_fut)
            .field("done_mapping", &self.done_mapping)
            .finish()
    }
}

impl<St, Fut, F, T> TryMapWhile<St, Fut, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> Fut,
    Fut: TryFuture<Ok = Option<T>, Error = St::Error>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, pending_fut: None, done_mapping: false }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, Fut, F, T> Stream for TryMapWhile<St, Fut, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> Fut,
    Fut: TryFuture<Ok = Option<T>, Error = St::Error>,
{
    type Item = Result<T, St::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done_mapping {
            return Poll::Ready(None);
        }

        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let res = ready!(fut.try_poll(cx));
                this.pending_fut.set(None);

                let mapped = res?;
                if mapped.is_none() {
                    *this.done_mapping = true;
                }

                break mapped.map(Ok);
            } else if let Some(item) = ready!(this.stream.as_mut().try_poll_next(cx)?) {
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

impl<St, Fut, F, T> FusedStream for TryMapWhile<St, Fut, F>
where
    St: TryStream + FusedStream,
    F: FnMut(St::Ok) -> Fut,
    Fut: TryFuture<Ok = Option<T>, Error = St::Error>,
{
    fn is_terminated(&self) -> bool {
        self.done_mapping || self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, F, Item, E> Sink<Item> for TryMapWhile<S, Fut, F>
where
    S: TryStream + Sink<Item, Error = E>,
{
    type Error = E;

    delegate_sink!(stream, Item);
}
