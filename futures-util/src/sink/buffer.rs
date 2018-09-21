use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::collections::VecDeque;
use std::marker::Unpin;
use std::pin::Pin;

/// Sink for the `Sink::buffer` combinator, which buffers up to some fixed
/// number of values when the underlying sink is unable to accept them.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Buffer<Si: Sink> {
    sink: Si,
    buf: VecDeque<Si::SinkItem>,

    // Track capacity separately from the `VecDeque`, which may be rounded up
    capacity: usize,
}

impl<Si: Sink + Unpin> Unpin for Buffer<Si> {}

impl<Si: Sink> Buffer<Si> {
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(buf: VecDeque<Si::SinkItem>);
    unsafe_unpinned!(capacity: usize);


    pub(super) fn new(sink: Si, capacity: usize) -> Buffer<Si> {
        Buffer {
            sink,
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &Si {
        &self.sink
    }

    fn try_empty_buffer(
        self: &mut Pin<&mut Self>,
        cx: &mut task::Context
    ) -> Poll<Result<(), Si::SinkError>> {
        try_ready!(self.sink().poll_ready(cx));
        while let Some(item) = self.buf().pop_front() {
            if let Err(e) = self.sink().start_send(item) {
                return Poll::Ready(Err(e));
            }
            if !self.buf.is_empty() {
                try_ready!(self.sink().poll_ready(cx));
            }
        }
        Poll::Ready(Ok(()))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S> Stream for Buffer<S> where S: Sink + Stream {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<Si: Sink> Sink for Buffer<Si> {
    type SinkItem = Si::SinkItem;
    type SinkError = Si::SinkError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        if *self.capacity() == 0 {
            return self.sink().poll_ready(cx);
        }

        if let Poll::Ready(Err(e)) = self.try_empty_buffer(cx) {
            return Poll::Ready(Err(e));
        }

        if self.buf().len() >= *self.capacity() {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        if *self.capacity() == 0 {
            self.sink().start_send(item)
        } else {
            self.buf().push_back(item);
            Ok(())
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.try_empty_buffer(cx));
        debug_assert!(self.buf().is_empty());
        self.sink().poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.try_empty_buffer(cx));
        debug_assert!(self.buf().is_empty());
        self.sink().poll_close(cx)
    }
}
