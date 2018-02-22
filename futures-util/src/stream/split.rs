use std::any::Any;
use std::error::Error;
use std::fmt;

use futures_core::{Stream, Poll, Async};
use futures_core::task;
use futures_sink::{ Sink};

use lock::BiLock;

/// A `Stream` part of the split pair
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct SplitStream<S>(BiLock<S>);

impl<S: Sink> SplitStream<S> {
    /// Attempts to put the two "halves" of a split `Stream + Sink` back
    /// together. Succeeds only if the `SplitStream<S>` and `SplitSink<S>` are
    /// a matching pair originating from the same call to `Stream::split`.
    pub fn reunite(self, other: SplitSink<S>) -> Result<S, ReuniteError<S>> {
        other.reunite(self)
    }
}

impl<S: Stream> Stream for SplitStream<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        match self.0.poll_lock(cx) {
            Async::Ready(mut inner) => inner.poll_next(cx),
            Async::Pending => Ok(Async::Pending),
        }
    }
}

#[allow(bad_style)]
fn SplitSink<S: Sink>(lock: BiLock<S>) -> SplitSink<S> {
    SplitSink {
        lock,
        slot: None,
    }
}

/// A `Sink` part of the split pair
#[derive(Debug)]
pub struct SplitSink<S: Sink> {
    lock: BiLock<S>,
    slot: Option<S::SinkItem>,
}

impl<S: Sink> SplitSink<S> {
    /// Attempts to put the two "halves" of a split `Stream + Sink` back
    /// together. Succeeds only if the `SplitStream<S>` and `SplitSink<S>` are
    /// a matching pair originating from the same call to `Stream::split`.
    pub fn reunite(self, other: SplitStream<S>) -> Result<S, ReuniteError<S>> {
        self.lock.reunite(other.0).map_err(|err| {
            ReuniteError(SplitSink(err.0), SplitStream(err.1))
        })
    }
}

impl<S: Sink> Sink for SplitSink<S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn poll_ready(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        loop {
            if self.slot.is_none() {
                return Ok(Async::Ready(()));
            }
            try_ready!(self.poll_flush(cx));
        }
    }

    fn start_send(&mut self, item: S::SinkItem) -> Result<(), S::SinkError> {
        self.slot = Some(item);
        Ok(())
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        match self.lock.poll_lock(cx) {
            Async::Ready(mut inner) => {
                if self.slot.is_some() {
                    try_ready!(inner.poll_ready(cx));
                    inner.start_send(self.slot.take().unwrap())?;
                }
                inner.poll_flush(cx)
            }
            Async::Pending => Ok(Async::Pending),
        }
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), S::SinkError> {
        match self.lock.poll_lock(cx) {
            Async::Ready(mut inner) => {
                if self.slot.is_some() {
                    try_ready!(inner.poll_ready(cx));
                    inner.start_send(self.slot.take().unwrap())?;
                }
                inner.poll_close(cx)
            }
            Async::Pending => Ok(Async::Pending),
        }
    }
}

pub fn split<S: Stream + Sink>(s: S) -> (SplitSink<S>, SplitStream<S>) {
    let (a, b) = BiLock::new(s);
    let read = SplitStream(a);
    let write = SplitSink(b);
    (write, read)
}

/// Error indicating a `SplitSink<S>` and `SplitStream<S>` were not two halves
/// of a `Stream + Split`, and thus could not be `reunite`d.
pub struct ReuniteError<T: Sink>(pub SplitSink<T>, pub SplitStream<T>);

impl<T: Sink> fmt::Debug for ReuniteError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("ReuniteError")
            .field(&"...")
            .finish()
    }
}

impl<T: Sink> fmt::Display for ReuniteError<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "tried to reunite a SplitStream and SplitSink that don't form a pair")
    }
}

impl<T: Any + Sink> Error for ReuniteError<T> {
    fn description(&self) -> &str {
        "tried to reunite a SplitStream and SplitSink that don't form a pair"
    }
}
