use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`wait_until`](super::StreamExt::wait_until) method.
    #[must_use = "streams do nothing unless polled"]
    #[derive(Debug)]
    pub struct WaitUntil<St: Stream, Fut: Future<Output = bool>>
    {
        is_fused: bool,
        #[pin]
        future: Option<Fut>,
        #[pin]
        stream: St,
    }
}

impl<St, Fut> WaitUntil<St, Fut>
where
    St: Stream,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, fut: Fut) -> Self {
        Self { stream, future: Some(fut), is_fused: false }
    }
}

impl<St, Fut> Stream for WaitUntil<St, Fut>
where
    St: Stream,
    Fut: Future<Output = bool>,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if *this.is_fused {
                break None;
            } else if let Some(future) = this.future.as_mut().as_pin_mut() {
                let ok = ready!(future.poll(cx));
                this.future.set(None);

                if !ok {
                    *this.is_fused = true;
                    break None;
                }
            } else {
                break ready!(this.stream.poll_next(cx));
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_fused {
            // No future values if it is fused
            (0, Some(0))
        } else {
            let (lower, upper) = self.stream.size_hint();

            if self.future.is_some() {
                // If future is not resolved yet, returns zero lower bound
                (0, upper)
            } else {
                // Returns size hint from underlying stream if the future is resolved
                (lower, upper)
            }
        }
    }
}

impl<St, Fut> FusedStream for WaitUntil<St, Fut>
where
    St: FusedStream,
    Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.is_fused || self.stream.is_terminated()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Fut, Item> Sink<Item> for WaitUntil<S, Fut>
where
    S: Stream + Sink<Item>,
    Fut: Future<Output = bool>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
