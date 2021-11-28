use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::pin_project;

/// Stream for the [`skip`](super::StreamExt::skip) method.
#[pin_project]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Skip<St> {
    #[pin]
    stream: St,
    remaining: usize,
}

impl<St: Stream> Skip<St> {
    pub(super) fn new(stream: St, n: usize) -> Skip<St> {
        Skip {
            stream,
            remaining: n,
        }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St: FusedStream> FusedStream for Skip<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: Stream> Stream for Skip<St> {
    type Item = St::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<St::Item>> {
        let mut this = self.project();

        while *this.remaining > 0 {
            if ready!(this.stream.as_mut().poll_next(cx)).is_some() {
                *this.remaining -= 1;
            } else {
                return Poll::Ready(None);
            }
        }

        this.stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.stream.size_hint();

        let lower = lower.saturating_sub(self.remaining as usize);
        let upper = match upper {
            Some(x) => Some(x.saturating_sub(self.remaining as usize)),
            None => None,
        };

        (lower, upper)
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Skip<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
