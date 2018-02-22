use core::marker::PhantomData;

use futures_core::{Async, Poll, Stream};
use futures_core::task;
use futures_sink::{Sink};

/// A stream combinator to change the error type of a stream.
///
/// This is created by the `Stream::err_into` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct ErrInto<S, E> {
    stream: S,
    f: PhantomData<E>
}

pub fn new<S, E>(stream: S) -> ErrInto<S, E>
    where S: Stream
{
    ErrInto {
        stream: stream,
        f: PhantomData
    }
}

impl<S, E> ErrInto<S, E> {
    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream
    }
}


impl<S: Stream, E> Stream for ErrInto<S, E>
    where S::Error: Into<E>,
{
    type Item = S::Item;
    type Error = E;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, E> {
        let e = match self.stream.poll_next(cx) {
            Ok(Async::Pending) => return Ok(Async::Pending),
            other => other,
        };
        e.map_err(Into::into)
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S: Stream + Sink, E> Sink for ErrInto<S, E> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;
    
    delegate_sink!(stream);
}
