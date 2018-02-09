use futures_core::{Async, Future, IntoFuture, Poll, Stream};
use futures_core::task;
use futures_sink::{Sink, StartSend};

/// A combinator used to filter the results of a stream and simultaneously map
/// them to a different type.
///
/// This structure is returned by the `Stream::filter_map` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct FilterMap<S, F, R>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: IntoFuture<Error=S::Error>,
{
    stream: S,
    f: F,
    pending: Option<R::Future>,
}

pub fn new<S, F, R>(s: S, f: F) -> FilterMap<S, F, R>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: IntoFuture<Error=S::Error>,
{
    FilterMap {
        stream: s,
        f: f,
        pending: None,
    }
}

impl<S, F, R> FilterMap<S, F, R>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: IntoFuture<Error=S::Error>,
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
impl<S, F, R> Sink for FilterMap<S, F, R>
    where S: Stream + Sink,
          F: FnMut(S::Item) -> R,
          R: IntoFuture<Error=S::Error>,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, ctx: &mut task::Context, item: S::SinkItem) -> StartSend<S::SinkItem, S::SinkError> {
        self.stream.start_send(ctx, item)
    }

    fn flush(&mut self, ctx: &mut task::Context) -> Poll<(), S::SinkError> {
        self.stream.flush(ctx)
    }

    fn close(&mut self, ctx: &mut task::Context) -> Poll<(), S::SinkError> {
        self.stream.close(ctx)
    }
}

impl<S, F, R, B> Stream for FilterMap<S, F, R>
    where S: Stream,
          F: FnMut(S::Item) -> R,
          R: IntoFuture<Item=Option<B>, Error=S::Error>,
{
    type Item = B;
    type Error = S::Error;

    fn poll(&mut self, ctx: &mut task::Context) -> Poll<Option<B>, S::Error> {
        loop {
            if self.pending.is_none() {
                let item = match try_ready!(self.stream.poll(ctx)) {
                    Some(e) => e,
                    None => return Ok(Async::Ready(None)),
                };
                let fut = ((self.f)(item)).into_future();
                self.pending = Some(fut);
            }

            match self.pending.as_mut().unwrap().poll(ctx) {
                x @ Ok(Async::Ready(Some(_))) => {
                    self.pending = None;
                    return x
                }
                Ok(Async::Ready(None)) => self.pending = None,
                Ok(Async::Pending) => return Ok(Async::Pending),
                Err(e) => {
                    self.pending = None;
                    return Err(e)
                }
            }
        }
    }
}
