use core::fmt;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, project};

use crate::fns::FnMut1;

/// Stream for the [`map`](super::StreamExt::map) method.
#[pin_project]
#[must_use = "streams do nothing unless polled"]
pub struct Map<St, F> {
    #[pin]
    stream: St,
    f: F,
}

impl<St, F> fmt::Debug for Map<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Map")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, F> Map<St, F> {
    pub(crate) fn new(stream: St, f: F) -> Map<St, F> {
        Map { stream, f }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St, F> FusedStream for Map<St, F>
    where St: FusedStream,
          F: FnMut1<St::Item>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F> Stream for Map<St, F>
    where St: Stream,
          F: FnMut1<St::Item>,
{
    type Item = F::Output;

    #[project]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        #[project]
        let Map { stream, f } = self.project();
        let res = ready!(stream.poll_next(cx));
        Poll::Ready(res.map(|x| f.call_mut(x)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<St, F, Item> Sink<Item> for Map<St, F>
    where St: Stream + Sink<Item>,
          F: FnMut1<St::Item>,
{
    type Error = St::Error;

    delegate_sink!(stream, Item);
}
