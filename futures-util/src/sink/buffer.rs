use alloc::collections::VecDeque;
use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};

/// Sink for the [`buffer`](super::SinkExt::buffer) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Buffer<Si: Sink<Item>, Item> {
    #[pin]
    sink: Si,
    buf: VecDeque<Item>,

    // Track capacity separately from the `VecDeque`, which may be rounded up
    capacity: usize,
}

impl<Si: Sink<Item>, Item> Buffer<Si, Item> {
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
    #[pin_project(self)]
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Si> {
        self.sink
    }

    /// Consumes this combinator, returning the underlying sink.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> Si {
        self.sink
    }

}

fn try_empty_buffer<Si: Sink<Item>, Item>(
    mut sink: Pin<&mut Si>,
    buf: &mut VecDeque<Item>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Si::Error>> {
    ready!(sink.as_mut().poll_ready(cx))?;
    while let Some(item) = buf.pop_front() {
        sink.as_mut().start_send(item)?;
        if !buf.is_empty() {
            ready!(sink.as_mut().poll_ready(cx))?;
        }
    }
    Poll::Ready(Ok(()))
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item> Stream for Buffer<S, Item> where S: Sink<Item> + Stream {
    type Item = S::Item;

    #[pin_project(self)]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        self.sink.poll_next(cx)
    }
}

impl<Si: Sink<Item>, Item> Sink<Item> for Buffer<Si, Item> {
    type Error = Si::Error;

    #[pin_project(self)]
    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if *self.capacity == 0 {
            return self.sink.as_mut().poll_ready(cx);
        }

        let _ = try_empty_buffer(self.sink.as_mut(), self.buf, cx)?;

        if self.buf.len() >= *self.capacity {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    #[pin_project(self)]
    fn start_send(
        mut self: Pin<&mut Self>,
        item: Item,
    ) -> Result<(), Self::Error> {
        if *self.capacity == 0 {
            self.sink.start_send(item)
        } else {
            self.buf.push_back(item);
            Ok(())
        }
    }

    #[pin_project(self)]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(try_empty_buffer(self.sink.as_mut(), self.buf, cx))?;
        debug_assert!(self.buf.is_empty());
        self.sink.poll_flush(cx)
    }

    #[pin_project(self)]
    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        ready!(try_empty_buffer(self.sink.as_mut(), self.buf, cx))?;
        debug_assert!(self.buf.is_empty());
        self.sink.poll_close(cx)
    }
}
