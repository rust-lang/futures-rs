use std::collections::VecDeque;

use futures_core::{Poll, Async, Stream};
use futures_core::task;
use futures_sink::Sink;

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

    fn try_empty_buffer(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        try_ready!(self.sink.poll_ready(cx));
        while let Some(item) = self.buf.pop_front() {
            self.sink.start_send(item)?;
            if self.buf.len() != 0 {
                try_ready!(self.sink.poll_ready(cx));
            }
        }
        Ok(Async::Ready(()))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S> Stream for Buffer<S> where S: Sink + Stream {
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll_next(cx)
    }
}

impl<S: Sink> Sink for Buffer<S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        if self.cap == 0 {
            return self.sink.poll_ready(cx);
        }

        self.try_empty_buffer(cx)?;

        Ok(if self.buf.len() >= self.cap {
            Async::Pending
        } else {
            Async::Ready(())
        })
    }

    fn start_send(&mut self, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        if self.cap == 0 {
            self.sink.start_send(item)
        } else {
            self.buf.push_back(item);
            Ok(())
        }
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        try_ready!(self.try_empty_buffer(cx));
        debug_assert!(self.buf.is_empty());
        self.sink.poll_flush(cx)
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        try_ready!(self.try_empty_buffer(cx));
        debug_assert!(self.buf.is_empty());
        self.sink.poll_close(cx)
    }
}
