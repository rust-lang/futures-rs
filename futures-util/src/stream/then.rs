use futures_core::{Async, IntoFuture, Future, Poll, Stream};
use futures_core::task;
use futures_sink::{ Sink};

/// A stream combinator which chains a computation onto each item produced by a
/// stream.
///
/// This structure is produced by the `Stream::then` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Then<S, U, F>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, U, F>(s: S, f: F) -> Then<S, U, F>
    where S: Stream,
          F: FnMut(Result<S::Item, S::Error>) -> U,
          U: IntoFuture,
{
    Then {
        stream: s,
        future: None,
        f: f,
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, U, F> Sink for Then<S, U, F>
    where S: Sink, U: IntoFuture,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}

impl<S, U, F> Stream for Then<S, U, F>
    where S: Stream,
          F: FnMut(Result<S::Item, S::Error>) -> U,
          U: IntoFuture,
{
    type Item = U::Item;
    type Error = U::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<U::Item>, U::Error> {
        if self.future.is_none() {
            let item = match self.stream.poll_next(cx) {
                Ok(Async::Pending) => return Ok(Async::Pending),
                Ok(Async::Ready(None)) => return Ok(Async::Ready(None)),
                Ok(Async::Ready(Some(e))) => Ok(e),
                Err(e) => Err(e),
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
