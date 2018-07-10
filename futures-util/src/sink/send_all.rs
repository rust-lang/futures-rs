use crate::stream::{StreamExt, Fuse};
use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// Future for the `Sink::send_all` combinator, which sends a stream of values
/// to a sink and then waits until the sink has fully flushed those values.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct SendAll<'a, Si, St>
where
    Si: Sink + Unpin + 'a + ?Sized,
    St: Stream + Unpin + 'a + ?Sized,
{
    sink: &'a mut Si,
    stream: Fuse<&'a mut St>,
    buffered: Option<Si::SinkItem>,
}

// Pinning is never projected to any fields
impl<'a, Si, St> Unpin for SendAll<'a, Si, St>
where
    Si: Sink + Unpin + 'a + ?Sized,
    St: Stream + Unpin + 'a + ?Sized,
{}

impl<'a, Si, St> SendAll<'a, Si, St>
where
    Si: Sink + Unpin + 'a + ?Sized,
    St: Stream<Item = Si::SinkItem> + Unpin + 'a + ?Sized,
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
        cx: &mut task::Context,
        item: Si::SinkItem,
    ) -> Poll<Result<(), Si::SinkError>> {
        debug_assert!(self.buffered.is_none());
        match PinMut::new(self.sink).poll_ready(cx) {
            Poll::Ready(Ok(())) => {
                Poll::Ready(PinMut::new(self.sink).start_send(item))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => {
                self.buffered = Some(item);
                Poll::Pending
            }
        }
    }
}

impl<'a, Si, St> Future for SendAll<'a, Si, St>
where
    Si: Sink + Unpin + 'a + ?Sized,
    St: Stream<Item = Si::SinkItem> + Unpin + 'a + ?Sized,
{
    type Output = Result<(), Si::SinkError>;

    fn poll(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        let this = &mut *self;
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = this.buffered.take() {
            try_ready!(this.try_start_send(cx, item))
        }

        loop {
            match this.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(item)) => {
                    try_ready!(this.try_start_send(cx, item))
                }
                Poll::Ready(None) => {
                    try_ready!(PinMut::new(this.sink).poll_flush(cx));
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    try_ready!(PinMut::new(this.sink).poll_flush(cx));
                    return Poll::Pending
                }
            }
        }
    }
}
