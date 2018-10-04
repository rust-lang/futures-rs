use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::pin::Pin;
use futures_core::task::{LocalWaker, Poll};
use futures_sink::Sink;
use pin_utils::unsafe_pinned;

/// Sink that clones incoming items and forwards them to two sinks at the same time.
///
/// Backpressure from any downstream sink propagates up, which means that this sink
/// can only process items as fast as its _slowest_ downstream sink.
pub struct Fanout<Si1: Sink, Si2: Sink> {
    sink1: Si1,
    sink2: Si2
}

impl<Si1: Sink, Si2: Sink> Fanout<Si1, Si2> {
    unsafe_pinned!(sink1: Si1);
    unsafe_pinned!(sink2: Si2);

    pub(super) fn new(sink1: Si1, sink2: Si2) -> Fanout<Si1, Si2> {
        Fanout { sink1, sink2 }
    }

    /// Consumes this combinator, returning the underlying sinks.
    ///
    /// Note that this may discard intermediate state of this combinator,
    /// so care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (Si1, Si2) {
        (self.sink1, self.sink2)
    }
}

impl<Si1: Sink + Debug, Si2: Sink + Debug> Debug for Fanout<Si1, Si2>
    where Si1::SinkItem: Debug,
          Si2::SinkItem: Debug
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        f.debug_struct("Fanout")
            .field("sink1", &self.sink1)
            .field("sink2", &self.sink2)
            .finish()
    }
}

impl<Si1, Si2> Sink for Fanout<Si1, Si2>
    where Si1: Sink,
          Si1::SinkItem: Clone,
          Si2: Sink<SinkItem=Si1::SinkItem, SinkError=Si1::SinkError>
{
    type SinkItem = Si1::SinkItem;
    type SinkError = Si1::SinkError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        let sink1_ready = try_poll!(self.sink1().poll_ready(lw)).is_ready();
        let sink2_ready = try_poll!(self.sink2().poll_ready(lw)).is_ready();
        let ready = sink1_ready && sink2_ready;
        if ready { Poll::Ready(Ok(())) } else { Poll::Pending }
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        self.sink1().start_send(item.clone())?;
        self.sink2().start_send(item)?;
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        let sink1_ready = try_poll!(self.sink1().poll_flush(lw)).is_ready();
        let sink2_ready = try_poll!(self.sink2().poll_flush(lw)).is_ready();
        let ready = sink1_ready && sink2_ready;
        if ready { Poll::Ready(Ok(())) } else { Poll::Pending }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        let sink1_ready = try_poll!(self.sink1().poll_close(lw)).is_ready();
        let sink2_ready = try_poll!(self.sink2().poll_close(lw)).is_ready();
        let ready = sink1_ready && sink2_ready;
        if ready { Poll::Ready(Ok(())) } else { Poll::Pending }
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use crate::future::FutureExt;
    use crate::sink::SinkExt;
    use crate::stream::{self, StreamExt};
    use futures_executor::block_on;
    use futures_channel::mpsc;
    use std::iter::Iterator;
    use std::vec::Vec;

    #[test]
    fn it_works() {
        let (tx1, rx1) = mpsc::channel(1);
        let (tx2, rx2) = mpsc::channel(2);
        let tx = tx1.fanout(tx2).sink_map_err(|_| ());

        let src = stream::iter((0..10).map(|x| Ok(x)));
        let fwd = src.forward(tx);

        let collect_fut1 = rx1.collect::<Vec<_>>();
        let collect_fut2 = rx2.collect::<Vec<_>>();
        let (_, vec1, vec2) = block_on(fwd.join3(collect_fut1, collect_fut2));

        let expected = (0..10).collect::<Vec<_>>();

        assert_eq!(vec1, expected);
        assert_eq!(vec2, expected);
    }
}
