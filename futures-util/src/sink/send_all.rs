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
pub struct SendAll<'a, T, U>
where
    T: Sink + Unpin + 'a + ?Sized,
    U: Stream + Unpin + 'a + ?Sized,
{
    sink: &'a mut T,
    stream: Fuse<&'a mut U>,
    buffered: Option<T::SinkItem>,
}

// Pinning is never projected to any fields
impl<'a, T, U> Unpin for SendAll<'a, T, U>
where
    T: Sink + Unpin + 'a + ?Sized,
    U: Stream + Unpin + 'a + ?Sized,
{}

pub fn new<'a, T, U>(sink: &'a mut T, stream: &'a mut U) -> SendAll<'a, T, U>
where
    T: Sink + Unpin + 'a + ?Sized,
    U: Stream<Item = Result<T::SinkItem, T::SinkError>> + Unpin + 'a + ?Sized,
{
    SendAll {
        sink,
        stream: stream.fuse(),
        buffered: None,
    }
}

impl<'a, T, U> SendAll<'a, T, U>
where
    T: Sink + Unpin + 'a + ?Sized,
    U: Stream<Item = Result<T::SinkItem, T::SinkError>> + Unpin + 'a + ?Sized,
{
    fn try_start_send(&mut self, cx: &mut task::Context, item: T::SinkItem)
        -> Poll<Result<(), T::SinkError>>
    {
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

impl<'a, T, U> Future for SendAll<'a, T, U>
where
    T: Sink + Unpin + 'a + ?Sized,
    U: Stream<Item = Result<T::SinkItem, T::SinkError>> + Unpin + 'a + ?Sized,
{
    type Output = Result<(), T::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let this = &mut *self;
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = this.buffered.take() {
            try_ready!(this.try_start_send(cx, item))
        }

        loop {
            match this.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(item))) => try_ready!(this.try_start_send(cx, item)),
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Err(err)),
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
