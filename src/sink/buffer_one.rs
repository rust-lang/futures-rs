use {Poll, Async};
use {StartSend, AsyncSink};
use sink::Sink;
use stream::Stream;

/// Adds buffering for a single value to the underlying sink.
///
/// Unlike `Sink::buffer`, this combinator is able to only buffer a single
/// value. As such, the buffer does not require any allocation. `BufferOne` also
/// provides `poll_ready`, which allows checking if sending a value will be
/// accepted.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct BufferOne<S: Sink> {
    sink: S,
    buf: Option<Result<S::SinkItem, S::SinkError>>,
}

impl<S: Sink> BufferOne<S> {
    /// Decorate `sink` with `BufferOne`.
    ///
    /// The returned `BufferOne` will have an empty buffer and will be
    /// guaranteed to be able to accept a single value.
    pub fn new(sink: S) -> BufferOne<S> {
        BufferOne {
            sink: sink,
            buf: None,
        }
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &S {
        &self.sink
    }

    /// Get a mutable reference to the inner sink.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Consumes the `BufferOne`, returning its underlying sink.
    ///
    /// If a value is currently buffered, it will be lost.
    pub fn into_inner(self) -> S {
        self.sink
    }

    /// Polls the sink to detect whether a call to `start_send` will be accepted
    /// or not.
    ///
    /// If this function returns `Async::Ready`, then a call to `start_send` is
    /// guaranted to return either `Ok(AsyncSink::Ready)` or `Err(_)`.
    ///
    /// If this function returns `Async::NotReady`, then the current task will
    /// become notified once the sink becomes ready to accept a new value.
    pub fn poll_ready(&mut self) -> Async<()> {
        match self.try_empty_buffer() {
            Err(e) => {
                self.buf = Some(Err(e));
                Async::Ready(())
            }
            Ok(ready) => ready,
        }
    }

    fn try_empty_buffer(&mut self) -> Poll<(), S::SinkError> {
        match self.buf.take() {
            Some(Ok(item)) => {
                match try!(self.sink.start_send(item)) {
                    AsyncSink::Ready => Ok(Async::Ready(())),
                    AsyncSink::NotReady(item) => {
                        self.buf = Some(Ok(item));

                        try!(self.sink.poll_complete());

                        Ok(Async::NotReady)
                    }
                }
            }
            Some(Err(e)) => Err(e),
            None => {
                Ok(Async::Ready(()))
            }
        }
    }
}

impl<S> Stream for BufferOne<S> where S: Sink + Stream {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        self.sink.poll()
    }
}

impl<S: Sink> Sink for BufferOne<S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        try!(self.try_empty_buffer());

        if self.buf.is_some() {
            return Ok(AsyncSink::NotReady(item));
        }

        self.buf = Some(Ok(item));

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.try_empty_buffer());
        debug_assert!(self.buf.is_none());
        self.sink.poll_complete()
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        if self.buf.is_some() {
            try_ready!(self.try_empty_buffer());
        }

        assert!(self.buf.is_none());

        self.sink.close()
    }
}
