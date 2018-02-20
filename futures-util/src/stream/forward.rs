use futures_core::{Async, Future, Poll, Stream};
use futures_core::task;
use futures_sink::{Sink};

use stream::{StreamExt, Fuse};

/// Future for the `Stream::forward` combinator, which sends a stream of values
/// to a sink and then waits until the sink has fully flushed those values.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Forward<T: Stream, U> {
    sink: Option<U>,
    stream: Option<Fuse<T>>,
    buffered: Option<T::Item>,
}


pub fn new<T, U>(stream: T, sink: U) -> Forward<T, U>
    where U: Sink<SinkItem=T::Item>,
          T: Stream,
          T::Error: From<U::SinkError>,
{
    Forward {
        sink: Some(sink),
        stream: Some(stream.fuse()),
        buffered: None,
    }
}

impl<T, U> Forward<T, U>
    where U: Sink<SinkItem=T::Item>,
          T: Stream,
          T::Error: From<U::SinkError>,
{
    /// Get a shared reference to the inner sink.
    /// If this combinator has already been polled to completion, None will be returned.
    pub fn sink_ref(&self) -> Option<&U> {
        self.sink.as_ref()
    }

    /// Get a mutable reference to the inner sink.
    /// If this combinator has already been polled to completion, None will be returned.
    pub fn sink_mut(&mut self) -> Option<&mut U> {
        self.sink.as_mut()
    }

    /// Get a shared reference to the inner stream.
    /// If this combinator has already been polled to completion, None will be returned.
    pub fn stream_ref(&self) -> Option<&T> {
        self.stream.as_ref().map(|x| x.get_ref())
    }

    /// Get a mutable reference to the inner stream.
    /// If this combinator has already been polled to completion, None will be returned.
    pub fn stream_mut(&mut self) -> Option<&mut T> {
        self.stream.as_mut().map(|x| x.get_mut())
    }

    fn take_result(&mut self) -> (T, U) {
        let sink = self.sink.take()
            .expect("Attempted to poll Forward after completion");
        let fuse = self.stream.take()
            .expect("Attempted to poll Forward after completion");
        (fuse.into_inner(), sink)
    }

    fn try_start_send(&mut self, cx: &mut task::Context, item: T::Item) -> Poll<(), U::SinkError> {
        debug_assert!(self.buffered.is_none());
        {
            let sink_mut = self.sink_mut().take().expect("Attempted to poll Forward after completion");
            if let Async::Ready(()) = sink_mut.poll_ready(cx)? {
                sink_mut.start_send(item)?;
                return Ok(Async::Ready(()))
            }
        }
        self.buffered = Some(item);
        Ok(Async::Pending)
    }
}

impl<T, U> Future for Forward<T, U>
    where U: Sink<SinkItem=T::Item>,
          T: Stream,
          T::Error: From<U::SinkError>,
{
    type Item = (T, U);
    type Error = T::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(T, U), T::Error> {
        // If we've got an item buffered already, we need to write it to the
        // sink before we can do anything else
        if let Some(item) = self.buffered.take() {
            try_ready!(self.try_start_send(cx, item))
        }

        loop {
            match self.stream_mut()
                .take().expect("Attempted to poll Forward after completion")
                .poll_next(cx)?
            {
                Async::Ready(Some(item)) => try_ready!(self.try_start_send(cx, item)),
                Async::Ready(None) => {
                    try_ready!(self.sink_mut().take().expect("Attempted to poll Forward after completion").poll_flush(cx));
                    return Ok(Async::Ready(self.take_result()))
                }
                Async::Pending => {
                    try_ready!(self.sink_mut().take().expect("Attempted to poll Forward after completion").poll_flush(cx));
                    return Ok(Async::Pending)
                }
            }
        }
    }
}
