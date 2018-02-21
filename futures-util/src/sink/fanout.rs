use core::fmt::{Debug, Formatter, Result as FmtResult};

use futures_core::{Async, Poll};
use futures_core::task;
use futures_sink::{ Sink};

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

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.left.keep_flushing(cx)?;
        self.right.keep_flushing(cx)?;
        if self.left.is_ready() && self.right.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.left.sink.start_send(item.clone())?;
        self.right.sink.start_send(item)?;
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_async = self.left.poll_flush(cx)?;
        let right_async = self.right.poll_flush(cx)?;

        if left_async.is_ready() && right_async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        let left_async = self.left.poll_close(cx)?;
        let right_async = self.right.poll_close(cx)?;

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
    state: Option<S::SinkItem>,
}

impl<S: Sink> Downstream<S> {
    fn new(sink: S) -> Self {
        Downstream { sink: sink, state: None }
    }

    fn is_ready(&self) -> bool {
        self.state.is_none()
    }

    fn keep_flushing(&mut self, cx: &mut task::Context) -> Result<(), S::SinkError> {
        if let Async::Ready(()) = self.sink.poll_ready(cx)? {
            if let Some(item) = self.state.take() {
                self.sink.start_send(item)?;
            }
        }
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        self.keep_flushing(cx)?;
        let async = self.sink.poll_flush(cx)?;

        // Only if all values have been sent _and_ the underlying
        // sink is completely flushed, signal readiness.
        if self.state.is_none() && async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        self.keep_flushing(cx)?;
        let async = self.sink.poll_flush(cx)?;

        if self.is_ready() {
            try_ready!(self.sink.poll_close(cx));
        }

        // Only if all values have been sent _and_ the underlying
        // sink is completely flushed, signal readiness.
        if self.state.is_none() && async.is_ready() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::Pending)
        }
    }
}
