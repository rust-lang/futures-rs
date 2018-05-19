use futures_core::{Poll, Future};
use futures_core::task;
use futures_sink::{Sink};

use core::marker::Unpin;
use core::mem::PinMut;

/// Future for the `Sink::send` combinator, which sends a value to a sink and
/// then waits until the sink has fully flushed.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Send<'a, S: Sink + 'a + ?Sized> {
    sink: PinMut<'a, S>,
    item: Option<S::SinkItem>,
}

// Pinning is never projected to children
impl<'a, S: Sink + ?Sized> Unpin for Send<'a, S> {}

pub fn new<'a, S: Sink + ?Sized>(sink: PinMut<'a, S>, item: S::SinkItem) -> Send<'a, S> {
    Send {
        sink,
        item: Some(item),
    }
}

impl<'a, S: Sink + ?Sized> Future for Send<'a, S> {
    type Output = Result<(), S::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        if let Some(item) = self.item.take() {
            match self.sink.reborrow().poll_ready(cx) {
                Poll::Ready(Ok(())) => {
                    if let Err(e) = self.sink.reborrow().start_send(item) {
                        return Poll::Ready(Err(e));
                    }
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    self.item = Some(item);
                    return Poll::Pending;
                }
            }
        }

        // we're done sending the item, but want to block on flushing the
        // sink
        try_ready!(self.sink.reborrow().poll_flush(cx));

        Poll::Ready(Ok(()))
    }
}
