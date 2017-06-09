use {Async, IntoFuture, Poll};
use stream::{Stream, Fuse, FuturesSet};

/// An adaptor for a stream of futures to execute the futures concurrently, if
/// possible.
///
/// This adaptor will buffer up a list of pending futures, and then return their
/// results in the order that they are returned by `Q`. This is created by the
/// `Stream::buffered` or the `Stream::buffer_unordered` method.
#[must_use = "streams do nothing unless polled"]
pub struct Buffered<Q, S>
    where S: Stream,
          S::Item: IntoFuture,
          Q: FuturesSet<<<S as Stream>::Item as IntoFuture>::Future>,
{
    stream: Fuse<S>,

    // Handles "in-flight" futures
    queue: Q,

    // Max number of futures
    max: usize,
}

if_std! {
use std::fmt;

impl<Q, S> fmt::Debug for Buffered<Q, S>
    where S: Stream + fmt::Debug,
          S::Item: IntoFuture,
          <<S as Stream>::Item as IntoFuture>::Future: fmt::Debug,
          Q: FuturesSet<<<S as Stream>::Item as IntoFuture>::Future> + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Buffered")
            .field("stream", &self.stream)
            .field("queue", &self.queue)
            .field("max", &self.max)
            .finish()
    }
}

}

pub fn new<Q, S>(s: S, amt: usize) -> Buffered<Q, S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
          Q: FuturesSet<<<S as Stream>::Item as IntoFuture>::Future>,
{
    Buffered {
        stream: super::fuse::new(s),
        queue: Q::new(),
        max: amt,
    }
}

impl<Q, S> Buffered<Q, S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
          Q: FuturesSet<<<S as Stream>::Item as IntoFuture>::Future>,
{
    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }
}

impl<Q, S> Stream for Buffered<Q, S>
    where S: Stream,
          S::Item: IntoFuture<Error=<S as Stream>::Error>,
          Q: FuturesSet<<<S as Stream>::Item as IntoFuture>::Future>,
{
    type Item = <S::Item as IntoFuture>::Item;
    type Error = <S as Stream>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // First up, try to spawn off as many futures as possible by filling up
        // our slab of futures.
        while self.queue.len() < self.max {
            let future = match try!(self.stream.poll()) {
                Async::Ready(Some(s)) => s.into_future(),
                Async::Ready(None) |
                Async::NotReady => break,
            };

            self.queue.push(future);
        }

        // Try polling a new future
        if let Some(val) = try_ready!(self.queue.poll()) {
            return Ok(Async::Ready(Some(val)));
        }

        // If we've gotten this far, then there are no events for us to process
        // and nothing was ready, so figure out if we're not done yet  or if
        // we've reached the end.
        if self.stream.is_done() {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}

// Forwarding impl of Sink from the underlying stream
impl<Q, S> ::sink::Sink for Buffered<Q, S>
    where S: ::sink::Sink + Stream,
          S::Item: IntoFuture,
          Q: FuturesSet<<<S as Stream>::Item as IntoFuture>::Future>,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, item: S::SinkItem) -> ::StartSend<S::SinkItem, S::SinkError> {
        self.stream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), S::SinkError> {
        self.stream.poll_complete()
    }

    fn close(&mut self) -> Poll<(), S::SinkError> {
        self.stream.close()
    }
}
