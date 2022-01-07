use core::fmt;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

use crate::fns::FnMut1;

pin_project! {
    /// Stream for the [`switch_map`](super::StreamExt::switch_map) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct SwitchMap<St, T, F> {
        #[pin]
        stream: St,
        #[pin]
        mapped_stream: Option<T>,
        f: F
    }
}

impl<St, T, F> fmt::Debug for SwitchMap<St, T, F>
where
    St: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwitchMap")
            .field("stream", &self.stream)
            .field("mapped_stream", &self.mapped_stream)
            .finish()
    }
}

impl<St, T, F> SwitchMap<St, T, F>
where
    St: Stream,
{
    pub(crate) fn new(stream: St, f: F) -> Self {
        Self { stream, mapped_stream: None, f }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, T, F> Stream for SwitchMap<St, T, F>
where
    St: Stream,
    T: Stream,
    F: FnMut1<St::Item, Output = T>,
{
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let source_pending = loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(data)) => {
                    this.mapped_stream.set(Some(this.f.call_mut(data)));
                }
                Poll::Pending => break true,
                _ => break false,
            }
        };
        let mapped_poll = this.mapped_stream.as_pin_mut().map(|stream| stream.poll_next(cx));
        match (source_pending, mapped_poll) {
            (_, Some(Poll::Ready(Some(s)))) => Poll::Ready(Some(s)),
            (false, None) | (false, Some(Poll::Ready(None))) => Poll::Ready(None),
            _ => Poll::Pending,
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, T, F> Sink<T> for SwitchMap<St, T, F>
where
    St: Stream + Sink<T>,
    T: Stream,
    F: FnMut1<St::Item>,
{
    type Error = St::Error;

    delegate_sink!(stream, T);
}
