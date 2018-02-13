use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::mem::replace;

use futures_core::{Async, Poll};
use futures_core::task;
use futures_sink::{AsyncSink, Sink, StartSend};

/// Sink that clones incoming items and forwards them to two sinks at the same time.
///
/// Backpressure from any downstream sink propagates up, which means that this sink
/// can only process items as fast as its _slowest_ downstream sink.
pub struct Fanout<A: Sink, B: Sink> {
    left: Downstream<A>,
    right: Downstream<B>
}

impl<A: Sink, B: Sink> Fanout<A, B> {
    /// Consumes this combinator, returning the underlying sinks.
    ///
    /// Note that this may discard intermediate state of this combinator,
    /// so care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (A, B) {
        (self.left.sink, self.right.sink)
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
    Fanout {
        left: Downstream::new(left),
        right: Downstream::new(right)
    }
}

impl<A, B> Sink for Fanout<A, B>
    where A: Sink,
          A::SinkItem: Clone,
          B: Sink<SinkItem=A::SinkItem, SinkError=A::SinkError>
{
    type SinkItem = A::SinkItem;
    type SinkError = A::SinkError;

    fn start_send(
        &mut self,
        ctx: &mut task::Context,
        item: Self::SinkItem
    ) -> StartSend<Self::SinkItem, Self::SinkError> {
        // Attempt to complete processing any outstanding requests.
        self.left.keep_flushing(ctx)?;
        self.right.keep_flushing(ctx)?;
        // Only if both downstream sinks are ready, start sending the next item.
        if self.left.is_ready() && self.right.is_ready() {
            self.left.state = self.left.sink.start_send(ctx, item.clone())?;
            self.right.state = self.right.sink.start_send(ctx, item)?;
            Ok(AsyncSink::Ready)
        } else {
            Ok(AsyncSink::Pending(item))
        }
    }

    fn flush(&mut self, ctx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_async = self.left.flush(ctx)?;
        let right_async = self.right.flush(ctx)?;
        // Only if both downstream sinks are ready, signal readiness.
        if left_async.is_ready() && right_async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }

    fn close(&mut self, ctx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_async = self.left.close(ctx)?;
        let right_async = self.right.close(ctx)?;
        // Only if both downstream sinks are ready, signal readiness.
        if left_async.is_ready() && right_async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }
}

#[derive(Debug)]
struct Downstream<S: Sink> {
    sink: S,
    state: AsyncSink<S::SinkItem>
}

impl<S: Sink> Downstream<S> {
    fn new(sink: S) -> Self {
        Downstream { sink: sink, state: AsyncSink::Ready }
    }

    fn is_ready(&self) -> bool {
        self.state.is_ready()
    }

    fn keep_flushing(&mut self, ctx: &mut task::Context) -> Result<(), S::SinkError> {
        if let AsyncSink::Pending(item) = replace(&mut self.state, AsyncSink::Ready) {
            self.state = self.sink.start_send(ctx, item)?;
        }
        Ok(())
    }

    fn flush(&mut self, ctx: &mut task::Context) -> Poll<(), S::SinkError> {
        self.keep_flushing(ctx)?;
        let async = self.sink.flush(ctx)?;
        // Only if all values have been sent _and_ the underlying
        // sink is completely flushed, signal readiness.
        if self.state.is_ready() && async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }

    fn close(&mut self, ctx: &mut task::Context) -> Poll<(), S::SinkError> {
        self.keep_flushing(ctx)?;
        // If all items have been flushed, initiate close.
        if self.state.is_ready() {
            self.sink.close(ctx)
        } else {
            Ok(Async::Pending)
        }
    }
}
