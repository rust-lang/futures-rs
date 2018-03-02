use core::marker::PhantomData;

use futures_core::{Poll, Async, Stream};
use futures_core::task;
use futures_sink::Sink;

/// Sink for the `Sink::with_flat_map` combinator, chaining a computation that returns an iterator
/// to run prior to pushing a value into the underlying sink
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = S::SinkItem, Error=S::SinkError>,
{
    sink: S,
    f: F,
    stream: Option<St>,
    buffer: Option<S::SinkItem>,
    _phantom: PhantomData<fn(U)>,
}

pub fn new<S, U, St, F>(sink: S, f: F) -> WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = S::SinkItem, Error=S::SinkError>,
{
    WithFlatMap {
        sink: sink,
        f: f,
        stream: None,
        buffer: None,
        _phantom: PhantomData,
    }
}

impl<S, U, St, F> WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = S::SinkItem, Error=S::SinkError>,
{
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

    fn try_empty_stream(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        if let Some(x) = self.buffer.take() {
            match self.sink.poll_ready(cx)? {
                Async::Ready(()) => self.sink.start_send(x)?,
                Async::Pending => {
                    self.buffer = Some(x);
                    return Ok(Async::Pending);
                }
            }
        }
        if let Some(mut stream) = self.stream.take() {
            while let Some(x) = try_ready!(stream.poll_next(cx)) {
                match self.sink.poll_ready(cx)? {
                    Async::Ready(()) => self.sink.start_send(x)?,
                    Async::Pending => {
                        self.stream = Some(stream);
                        self.buffer = Some(x);
                        return Ok(Async::Pending);
                    }
                }
            }
        }
        Ok(Async::Ready(()))
    }
}

impl<S, U, St, F> Stream for WithFlatMap<S, U, St, F>
where
    S: Stream + Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = S::SinkItem, Error=S::SinkError>,
{
    type Item = S::Item;
    type Error = S::Error;
    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll_next(cx)
    }
}

impl<S, U, St, F> Sink for WithFlatMap<S, U, St, F>
where
    S: Sink,
    F: FnMut(U) -> St,
    St: Stream<Item = S::SinkItem, Error=S::SinkError>,
{
    type SinkItem = U;
    type SinkError = S::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        self.try_empty_stream(cx)
    }

    fn start_send(&mut self, i: Self::SinkItem) -> Result<(), Self::SinkError> {
        assert!(self.stream.is_none());
        self.stream = Some((self.f)(i));
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        if self.try_empty_stream(cx)?.is_pending() {
            return Ok(Async::Pending);
        }
        self.sink.poll_flush(cx)
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::SinkError> {
        if self.try_empty_stream(cx)?.is_pending() {
            return Ok(Async::Pending);
        }
        self.sink.poll_close(cx)
    }
}
