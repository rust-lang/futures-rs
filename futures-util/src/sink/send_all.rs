use futures_core::{Poll, Future, Stream};
use futures_core::task;
use futures_sink::{Sink};

use stream::{StreamExt, Fuse};

use core::marker::Unpin;
use core::mem::PinMut;

/// Future for the `Sink::send_all` combinator, which sends a stream of values
/// to a sink and then waits until the sink has fully flushed those values.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct SendAll<'a, T: 'a + ?Sized + Sink, U: Stream + 'a + ?Sized> {
    sink: PinMut<'a, T>,
    stream: Fuse<PinMut<'a, U>>,
    buffered: Option<T::SinkItem>,
}

// Pinning is never projected to any fields
impl<'a, T: ?Sized + Sink, U: Stream + ?Sized> Unpin for SendAll<'a, T, U> {}

pub fn new<'a, T, U, E>(sink: PinMut<'a, T>, stream: PinMut<'a, U>) -> SendAll<'a, T, U>
    where T: Sink + ?Sized,
          U: Stream<Item = Result<T::SinkItem, E>> + ?Sized,
          T::SinkError: From<E>,
{
    SendAll {
        sink,
        stream: stream.fuse(),
        buffered: None,
    }
}

impl<'a, T, U, E> SendAll<'a, T, U>
    where T: Sink + ?Sized,
          U: Stream<Item = Result<T::SinkItem, E>> + ?Sized,
          T::SinkError: From<E>,
{
    fn try_start_send(self: &mut PinMut<Self>, cx: &mut task::Context, item: T::SinkItem)
        -> Poll<Result<(), T::SinkError>>
    {
        debug_assert!(self.buffered.is_none());
        match self.sink.reborrow().poll_ready(cx) {
            Poll::Ready(Ok(())) => {
                Poll::Ready(self.sink.reborrow().start_send(item).map_err(Into::into))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => {
                self.buffered = Some(item);
                Poll::Pending
            }
        }
    }
}

impl<'a, T, U, E> Future for SendAll<'a, T, U>
    where T: Sink,
          U: Stream<Item = Result<T::SinkItem, E>>,
          T::SinkError: From<E>,
{
    type Output = Result<(), T::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = self.buffered.take() {
            try_ready!(self.try_start_send(cx, item))
        }

        loop {
            match self.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(item))) => try_ready!(self.try_start_send(cx, item)),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e.into())),
                Poll::Ready(None) => {
                    try_ready!(self.sink.reborrow().poll_flush(cx));
                    return Poll::Ready(Ok(()))
                }
                Poll::Pending => {
                    try_ready!(self.sink.reborrow().poll_flush(cx));
                    return Poll::Pending
                }
            }
        }
    }
}
