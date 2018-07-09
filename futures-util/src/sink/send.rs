use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

/// Future for the `Sink::send` combinator, which sends a value to a sink and
/// then waits until the sink has fully flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Send<'a, S: Sink + Unpin + 'a + ?Sized> {
    sink: &'a mut S,
    item: Option<S::SinkItem>,
}

// Pinning is never projected to children
impl<'a, S: Sink + Unpin + ?Sized> Unpin for Send<'a, S> {}

pub fn new<'a, S: Sink + Unpin + ?Sized>(sink: &'a mut S, item: S::SinkItem) -> Send<'a, S> {
    Send {
        sink,
        item: Some(item),
    }
}

impl<'a, S: Sink + Unpin + ?Sized> Future for Send<'a, S> {
    type Output = Result<(), S::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let this = &mut *self;
        if let Some(item) = this.item.take() {
            let mut sink = PinMut::new(this.sink);
            match sink.reborrow().poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    if let Err(e) = sink.reborrow().start_send(item) {
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
        try_ready!(PinMut::new(this.sink).poll_flush(cx));

        Poll::Ready(Ok(()))
    }
}
