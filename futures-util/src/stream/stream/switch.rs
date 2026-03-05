use core::pin::Pin;

use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_core::FusedStream;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`switch`](super::StreamExt::switch) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Switch<Outer, Inner> {
        #[pin]
        outer_stream: Outer,
        outer_stream_is_closed: bool,
        #[pin]
        inner_stream: Option<Inner>,
    }
}

impl<Outer: Stream> Switch<Outer, Outer::Item> {
    /// Creates a new [`Switch`] stream.
    pub(super) fn new(outer_stream: Outer) -> Self {
        Self { outer_stream, outer_stream_is_closed: false, inner_stream: None }
    }

    delegate_access_inner!(outer_stream, Outer, ());
}

impl<Outer> FusedStream for Switch<Outer, Outer::Item>
where
    Outer: FusedStream,
    Outer::Item: Stream,
{
    fn is_terminated(&self) -> bool {
        self.inner_stream.is_none() && self.outer_stream.is_terminated()
    }
}

impl<Outer> Stream for Switch<Outer, Outer::Item>
where
    Outer: Stream,
    Outer::Item: Stream,
{
    type Item = <Outer::Item as Stream>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // Poll the latest inner stream eagerly.
        if !*this.outer_stream_is_closed {
            while let Poll::Ready(ready) = this.outer_stream.as_mut().poll_next(cx) {
                match ready {
                    Some(inner_stream) => {
                        this.inner_stream.set(Some(inner_stream));
                    }

                    None => {
                        *this.outer_stream_is_closed = true;
                        break;
                    }
                }
            }
        }

        match this.inner_stream.as_mut().as_pin_mut() {
            // No inner stream has been produced yet.
            None => {
                // The stream' state is the outer stream' state.
                if *this.outer_stream_is_closed {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }

            // An inner stream exists: poll it!
            Some(inner_stream) => match inner_stream.poll_next(cx) {
                // Inner stream produced an item.
                Poll::Ready(Some(item)) => Poll::Ready(Some(item)),

                // Both inner and outer streams are closed.
                Poll::Ready(None) if *this.outer_stream_is_closed => Poll::Ready(None),

                // Only inner stream is closed or is pending.
                Poll::Ready(None) | Poll::Pending => Poll::Pending,
            },
        }
    }
}
