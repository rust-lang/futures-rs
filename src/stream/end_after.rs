use {Async, Poll};
use stream::Stream;

/// A stream combinator used to end the stream after the predicate returns
/// true for an element. The stream yields all of the elements for which
/// the predicate returns false and then yields the element for which the
/// predicate returned true as the stream's final element.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct EndAfter<S, P> {
    stream: S,
    pred: P,
    ended: bool,
}

pub fn new<S, P>(s: S, p: P) -> EndAfter<S, P>
    where S: Stream,
          P: FnMut(&S::Item) -> bool,
{
    EndAfter {
        stream: s,
        pred: p,
        ended: false,
    }
}

impl<S, F> EndAfter<S, F> {
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
impl<S, P> ::sink::Sink for EndAfter<S, P>
    where S: ::sink::Sink + Stream
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

impl<S, P> Stream for EndAfter<S, P>
    where S: Stream,
          P: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        if self.ended {
            return Ok(Async::Ready(None));
        }

        match try_ready!(self.stream.poll()) {
            Some(e) => {
                if (self.pred)(&e) {
                    self.ended = true;

                }
                Ok(Async::Ready(Some(e)))
            },
            None => Ok(Async::Ready(None)),
        }
    }
}
