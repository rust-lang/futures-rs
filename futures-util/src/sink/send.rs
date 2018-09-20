use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// Future for the `Sink::send` combinator, which sends a value to a sink and
/// then waits until the sink has fully flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Send<'a, Si: Sink + Unpin + 'a + ?Sized> {
    sink: &'a mut Si,
    item: Option<Si::SinkItem>,
}

// Pinning is never projected to children
impl<Si: Sink + Unpin + ?Sized> Unpin for Send<'_, Si> {}

impl<'a, Si: Sink + Unpin + ?Sized> Send<'a, Si> {
    pub(super) fn new(sink: &'a mut Si, item: Si::SinkItem) -> Self {
        Send {
            sink,
            item: Some(item),
        }
    }
}

impl<Si: Sink + Unpin + ?Sized> Future for Send<'_, Si> {
    type Output = Result<(), Si::SinkError>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Self::Output> {
        let this = &mut *self;
        if let Some(item) = this.item.take() {
            let mut sink = Pin::new(&mut this.sink);
            match sink.as_mut().poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    if let Err(e) = sink.as_mut().start_send(item) {
                        return Poll::Ready(Err(e));
                    }
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    this.item = Some(item);
                    return Poll::Pending;
                }
            }
        }

        // we're done sending the item, but want to block on flushing the
        // sink
        try_ready!(Pin::new(&mut this.sink).poll_flush(cx));

        Poll::Ready(Ok(()))
    }
}
