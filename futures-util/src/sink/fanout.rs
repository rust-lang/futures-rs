use core::fmt::{Debug, Formatter, Result as FmtResult};

use futures_core::{Async, Poll};
use futures_core::task;
use futures_sink::{ Sink};

/// Sink that clones incoming items and forwards them to two sinks at the same time.
///
/// Backpressure from any downstream sink propagates up, which means that this sink
/// can only process items as fast as its _slowest_ downstream sink.
pub struct Fanout<A: Sink, B: Sink> {
    left: A,
    right: B
}

impl<A: Sink, B: Sink> Fanout<A, B> {
    /// Consumes this combinator, returning the underlying sinks.
    ///
    /// Note that this may discard intermediate state of this combinator,
    /// so care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (A, B) {
        (self.left, self.right)
    }
}

impl<A: Sink + Debug, B: Sink + Debug> Debug for Fanout<A, B>
    where A::SinkItem: Debug,
          B::SinkItem: Debug
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        f.debug_struct("Fanout")
            .field("left", &self.left)
            .field("right", &self.right)
            .finish()
    }
}

pub fn new<A: Sink, B: Sink>(left: A, right: B) -> Fanout<A, B> {
    Fanout {left, right}
}

impl<A, B> Sink for Fanout<A, B>
    where A: Sink,
          A::SinkItem: Clone,
          B: Sink<SinkItem=A::SinkItem, SinkError=A::SinkError>
{
    type SinkItem = A::SinkItem;
    type SinkError = A::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_ready = self.left.poll_ready(cx)?.is_ready();
        let right_ready = self.right.poll_ready(cx)?.is_ready();
        let ready = left_ready && right_ready;
        Ok(if ready {Async::Ready(())} else {Async::Pending})
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.left.start_send(item.clone())?;
        self.right.start_send(item)?;
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_ready = self.left.poll_flush(cx)?.is_ready();
        let right_ready = self.right.poll_flush(cx)?.is_ready();
        let ready = left_ready && right_ready;
        Ok(if ready {Async::Ready(())} else {Async::Pending})
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_ready = self.left.poll_close(cx)?.is_ready();
        let right_ready = self.right.poll_close(cx)?.is_ready();
        let ready = left_ready && right_ready;
        Ok(if ready {Async::Ready(())} else {Async::Pending})
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use future::FutureExt;
    use futures_channel::mpsc;
    use futures_executor::block_on;
    use sink::SinkExt;
    use std::iter::Iterator;
    use std::vec::Vec;
    use stream::{self, StreamExt};

    #[test]
    fn it_works() {
        let src = stream::iter_ok::<_, ()>(0..10);
        let (sender0, receiver0) = mpsc::channel(1);
        let (sender1, receiver1) = mpsc::channel(2);
        let senders = sender0.fanout(sender1).sink_map_err(|_| ());
        let fwd = src.forward(senders).map(|_| ());
        let list0 = receiver0.collect::<Vec<_>>().map_err(|_| ());
        let list1 = receiver1.collect::<Vec<_>>().map_err(|_| ());
        let lists = fwd.join(list0.join(list1)).map(|(_, lists)| lists);
        let lists = block_on(lists);
        let lists = lists.as_ref()
            .map(|&(ref list0, ref list1)| (&**list0, &**list1));
        let expected = (0..10).collect::<Vec<_>>();
        assert_eq!(lists, Ok((&*expected, &*expected)));
    }
}
