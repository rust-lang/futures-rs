use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use core::pin::Pin;
use alloc::collections::VecDeque;

/// Sink for the [`buffer`](super::SinkExt::buffer) method.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Buffer<Si: Sink<Item>, Item> {
    sink: Si,
    buf: VecDeque<Item>,

    // Track capacity separately from the `VecDeque`, which may be rounded up
    capacity: usize,
}

impl<Si: Sink<Item> + Unpin, Item> Unpin for Buffer<Si, Item> {}

impl<Si: Sink<Item>, Item> Buffer<Si, Item> {
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(buf: VecDeque<Item>);
    unsafe_unpinned!(capacity: usize);

    pub(super) fn new(sink: Si, capacity: usize) -> Self {
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

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut Si {
        &mut self.sink
    }

    /// Get a pinned mutable reference to the inner sink.
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Si> {
        self.sink()
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> Si {
        self.sink
    }

    fn try_empty_buffer(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Si::SinkError>> {
        try_ready!(self.as_mut().sink().poll_ready(cx));
        while let Some(item) = self.as_mut().buf().pop_front() {
            if let Err(e) = self.as_mut().sink().start_send(item) {
                return Poll::Ready(Err(e));
            }
            if !self.buf.is_empty() {
                try_ready!(self.as_mut().sink().poll_ready(cx));
            }
        }
        Poll::Ready(Ok(()))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item> Stream for Buffer<S, Item> where S: Sink<Item> + Stream {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<Si: Sink<Item>, Item> Sink<Item> for Buffer<Si, Item> {
    type SinkError = Si::SinkError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        if self.capacity == 0 {
            return self.as_mut().sink().poll_ready(cx);
        }

        if let Poll::Ready(Err(e)) = self.as_mut().try_empty_buffer(cx) {
            return Poll::Ready(Err(e));
        }

        if self.buf.len() >= self.capacity {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Item,
    ) -> Result<(), Self::SinkError> {
        if self.capacity == 0 {
            self.as_mut().sink().start_send(item)
        } else {
            self.as_mut().buf().push_back(item);
            Ok(())
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.as_mut().try_empty_buffer(cx));
        debug_assert!(self.as_mut().buf().is_empty());
        self.as_mut().sink().poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.as_mut().try_empty_buffer(cx));
        debug_assert!(self.as_mut().buf().is_empty());
        self.as_mut().sink().poll_close(cx)
    }
}
