use crate::stream::{StreamExt, Fuse};
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;

/// Future for the [`send_all`](super::SinkExt::send_all) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct SendAll<'a, Si, St>
where
    Si: Sink<St::Item> + Unpin + ?Sized,
    St: Stream + Unpin + ?Sized,
{
    sink: &'a mut Si,
    stream: Fuse<&'a mut St>,
    buffered: Option<St::Item>,
}

// Pinning is never projected to any fields
impl<Si, St> Unpin for SendAll<'_, Si, St>
where
    Si: Sink<St::Item> + Unpin + ?Sized,
    St: Stream + Unpin + ?Sized,
{}

impl<'a, Si, St> SendAll<'a, Si, St>
where
    Si: Sink<St::Item> + Unpin + ?Sized,
    St: Stream + Unpin + ?Sized,
{
    pub(super) fn new(
        sink: &'a mut Si,
        stream: &'a mut St,
    ) -> SendAll<'a, Si, St> {
        SendAll {
            sink,
            stream: stream.fuse(),
            buffered: None,
        }
    }

    fn try_start_send(
        &mut self,
        cx: &mut Context<'_>,
        item: St::Item,
    ) -> Poll<Result<(), Si::Error>> {
        debug_assert!(self.buffered.is_none());
        match Pin::new(&mut self.sink).poll_ready(cx)? {
            Poll::Ready(()) => {
                Poll::Ready(Pin::new(&mut self.sink).start_send(item))
            }
            Poll::Pending => {
                self.buffered = Some(item);
                Poll::Pending
            }
        }
    }
}

impl<Si, St> Future for SendAll<'_, Si, St>
where
    Si: Sink<St::Item> + Unpin + ?Sized,
    St: Stream + Unpin + ?Sized,
{
    type Output = Result<(), Si::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let this = &mut *self;
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = this.buffered.take() {
            ready!(this.try_start_send(cx, item))?
        }

        loop {
            match this.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(item)) => {
                    ready!(this.try_start_send(cx, item))?
                }
                Poll::Ready(None) => {
                    ready!(Pin::new(&mut this.sink).poll_flush(cx))?;
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    ready!(Pin::new(&mut this.sink).poll_flush(cx))?;
                    return Poll::Pending
                }
            }
        }
    }
}
