use std::collections::VecDeque;

use futures_core::{Poll, Async, Stream};
use futures_core::task;
use futures_sink::{StartSend, AsyncSink, Sink};

/// Sink for the `Sink::buffer` combinator, which buffers up to some fixed
/// number of values when the underlying sink is unable to accept them.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Buffer<S: Sink> {
    sink: S,
    buf: VecDeque<S::SinkItem>,

    // Track capacity separately from the `VecDeque`, which may be rounded up
    cap: usize,
}

pub fn new<S: Sink>(sink: S, amt: usize) -> Buffer<S> {
    Buffer {
        sink: sink,
        buf: VecDeque::with_capacity(amt),
        cap: amt,
    }
}

impl<S: Sink> Buffer<S> {
    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.sink
    }

    fn try_empty_buffer(&mut self, ctx: &mut task::Context) -> Poll<(), S::SinkError> {
        while let Some(item) = self.buf.pop_front() {
            if let AsyncSink::Pending(item) = self.sink.start_send(ctx, item)? {
                self.buf.push_front(item);

                // ensure that we attempt to complete any pushes we've started
                self.sink.flush(ctx)?;

                return Ok(Async::Pending);
            }
        }

        Ok(Async::Ready(()))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S> Stream for Buffer<S> where S: Sink + Stream {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, ctx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll(ctx)
    }
}

impl<S: Sink> Sink for Buffer<S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, ctx: &mut task::Context, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        if self.cap == 0 {
            return self.sink.start_send(ctx, item);
        }

        self.try_empty_buffer(ctx)?;
        if self.buf.len() == self.cap {
            return Ok(AsyncSink::Pending(item));
        }
        self.buf.push_back(item);
        Ok(AsyncSink::Ready)
    }

    fn flush(&mut self, ctx: &mut task::Context) -> Poll<(), Self::SinkError> {
        if self.cap == 0 {
            return self.sink.flush(ctx);
        }

        try_ready!(self.try_empty_buffer(ctx));
        debug_assert!(self.buf.is_empty());
        self.sink.flush(ctx)
    }

    fn close(&mut self, ctx: &mut task::Context) -> Poll<(), Self::SinkError> {
        if self.cap == 0 {
            return self.sink.close(ctx);
        }

        if self.buf.len() > 0 {
            try_ready!(self.try_empty_buffer(ctx));
        }
        assert_eq!(self.buf.len(), 0);
        self.sink.close(ctx)
    }
}
