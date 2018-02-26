use futures_core::{IntoFuture, Future, Poll, Async, Stream};
use futures_core::task;
use futures_sink::{Sink};

/// A stream combinator which chains a computation onto values produced by a
/// stream.
///
/// This structure is produced by the `Stream::and_then` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct AndThen<S, U, F>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, U, F>(s: S, f: F) -> AndThen<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Error=S::Error>,
{
    AndThen {
        stream: s,
        future: None,
        f: f,
    }
}

impl<S, U, F> AndThen<S, U, F>
    where U: IntoFuture,
{
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

// Forwarding impl of Sink from the underlying stream
impl<S, U: IntoFuture, F> Sink for AndThen<S, U, F>
    where S: Sink
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}

impl<S, U, F> Stream for AndThen<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: IntoFuture<Error=S::Error>,
{
    type Item = U::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<U::Item>, S::Error> {
        if self.future.is_none() {
            let item = match try_ready!(self.stream.poll_next(cx)) {
                None => return Ok(Async::Ready(None)),
                Some(e) => e,
            };
            self.future = Some((self.f)(item).into_future());
        }
        assert!(self.future.is_some());
        match self.future.as_mut().unwrap().poll(cx) {
            Ok(Async::Ready(e)) => {
                self.future = None;
                Ok(Async::Ready(Some(e)))
            }
            Err(e) => {
                self.future = None;
                Err(e)
            }
            Ok(Async::Pending) => Ok(Async::Pending)
        }
    }
}
