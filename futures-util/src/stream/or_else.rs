use futures_core::{IntoFuture, Future, Poll, Async, Stream};
use futures_core::task;
use futures_sink::{Sink};

/// A stream combinator which chains a computation onto errors produced by a
/// stream.
///
/// This structure is produced by the `Stream::or_else` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct OrElse<S, U, F>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, U, F>(s: S, f: F) -> OrElse<S, U, F>
    where S: Stream,
          F: FnMut(S::Error) -> U,
          U: IntoFuture<Item=S::Item>,
{
    OrElse {
        stream: s,
        future: None,
        f: f,
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, U, F> Sink for OrElse<S, U, F>
    where S: Sink, U: IntoFuture
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}

impl<S, U, F> Stream for OrElse<S, U, F>
    where S: Stream,
          F: FnMut(S::Error) -> U,
          U: IntoFuture<Item=S::Item>,
{
    type Item = S::Item;
    type Error = U::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, U::Error> {
        if self.future.is_none() {
            let item = match self.stream.poll_next(cx) {
                Ok(Async::Ready(e)) => return Ok(Async::Ready(e)),
                Ok(Async::Pending) => return Ok(Async::Pending),
                Err(e) => e,
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
