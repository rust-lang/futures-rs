use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use futures_sink::Sink;
use std::collections::VecDeque;
use std::marker::Unpin;
use std::mem::PinMut;

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
        sink,
        buf: VecDeque::with_capacity(amt),
        cap: amt,
    }
}

impl<S: Sink + Unpin> Unpin for Buffer<S> {}

impl<S: Sink> Buffer<S> {
    unsafe_pinned!(sink -> S);
    unsafe_unpinned!(buf -> VecDeque<S::SinkItem>);
    unsafe_unpinned!(cap -> usize);

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        &self.sink
    }

    fn try_empty_buffer(self: &mut PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), S::SinkError>> {
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

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        self.sink().poll_next(cx)
    }
}

impl<S: Sink> Sink for Buffer<S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        if *self.cap() == 0 {
            return self.sink().poll_ready(cx);
        }

        if let Poll::Ready(Err(e)) = self.try_empty_buffer(cx) {
            return Poll::Ready(Err(e));
        }

        if self.buf().len() >= *self.cap() {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(mut self: PinMut<Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        if *self.cap() == 0 {
            self.sink().start_send(item)
        } else {
            self.buf().push_back(item);
            Ok(())
        }
    }

    fn poll_flush(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.try_empty_buffer(cx));
        debug_assert!(self.buf().is_empty());
        self.sink().poll_flush(cx)
    }

    fn poll_close(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.try_empty_buffer(cx));
        debug_assert!(self.buf().is_empty());
        self.sink().poll_close(cx)
    }
}
