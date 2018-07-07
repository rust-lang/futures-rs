use crate::stream::{StreamExt, Fuse};
use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Poll, Context};
use futures_sink::Sink;

const INVALID_POLL: &str = "polled `Forward` after completion";

/// Future for the `Stream::forward` combinator, which sends a stream of values
/// to a sink and then flushes the sink.
///
/// Note: this is only usable with `Unpin` sinks, so `Sink`s that aren't `Unpin`
/// will need to be pinned in order to be used with this combinator.
//
// This limitation is necessary in order to return the sink after the forwarding
// has completed so that it can be used again.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Forward<T: Stream, U: Sink + Unpin> {
    sink: Option<U>,
    stream: Fuse<T>,
    buffered: Option<U::SinkItem>,
}

impl<T: Stream + Unpin, U: Sink + Unpin> Unpin for Forward<T, U> {}

pub fn new<T, U>(stream: T, sink: U) -> Forward<T, U>
where
    U: Sink + Unpin,
    T: Stream<Item = Result<U::SinkItem, U::SinkError>>,
{
    Forward {
        sink: Some(sink),
        stream: stream.fuse(),
        buffered: None,
    }
}

impl<T, U> Forward<T, U>
where
    U: Sink + Unpin,
    T: Stream<Item = Result<U::SinkItem, U::SinkError>>,
{
    unsafe_pinned!(sink -> Option<U>);
    unsafe_pinned!(stream -> Fuse<T>);
    unsafe_unpinned!(buffered -> Option<U::SinkItem>);

    fn try_start_send(
        mut self: PinMut<Self>,
        cx: &mut Context,
        item: U::SinkItem,
    ) -> Poll<Result<(), U::SinkError>> {
        debug_assert!(self.buffered.is_none());
        {
            let mut sink = self.sink().as_pin_mut().unwrap();
            if try_poll!(sink.reborrow().poll_ready(cx)).is_ready() {
                return Poll::Ready(sink.start_send(item));
            }
        }
        *self.buffered() = Some(item);
        Poll::Pending
    }
}

impl<T, U> Future for Forward<T, U>
where
    U: Sink + Unpin,
    T: Stream<Item = Result<U::SinkItem, U::SinkError>>,
{
    type Output = Result<U, U::SinkError>;

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<Self::Output> {
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = self.buffered().take() {
            try_ready!(self.reborrow().try_start_send(cx, item));
        }

        loop {
            match self.stream().poll_next(cx) {
                Poll::Ready(Some(Ok(item))) => try_ready!(self.reborrow().try_start_send(cx, item)),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e)),
                Poll::Ready(None) => {
                    try_ready!(self.sink().as_pin_mut().expect(INVALID_POLL).poll_close(cx));
                    return Poll::Ready(Ok(self.sink().take().unwrap()))
                }
                Poll::Pending => {
                    try_ready!(self.sink().as_pin_mut().expect(INVALID_POLL).poll_flush(cx));
                    return Poll::Pending
                }
            }
        }
    }
}
