use futures_core::{Async, Poll, Stream};
use futures_core::task;
use futures_sink::{Sink};

use stream::{StreamExt, Fuse};

/// A `Stream` that implements a `peek` method.
///
/// The `peek` method can be used to retrieve a reference
/// to the next `Stream::Item` if available. A subsequent
/// call to `poll` will return the owned item.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Peekable<S: Stream> {
    stream: Fuse<S>,
    peeked: Option<S::Item>,
}


pub fn new<S: Stream>(stream: S) -> Peekable<S> {
    Peekable {
        stream: stream.fuse(),
        peeked: None
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Peekable<S>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}

impl<S: Stream> Stream for Peekable<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(item) = self.peeked.take() {
            return Ok(Async::Ready(Some(item)))
        }
        self.stream.poll_next(cx)
    }
}


impl<S: Stream> Peekable<S> {
    /// Peek retrieves a reference to the next item in the stream.
    ///
    /// This method polls the underlying stream and return either a reference
    /// to the next item if the stream is ready or passes through any errors.
    pub fn peek(&mut self, cx: &mut task::Context) -> Poll<Option<&S::Item>, S::Error> {
        if self.peeked.is_some() {
            return Ok(Async::Ready(self.peeked.as_ref()))
        }
        match try_ready!(self.poll_next(cx)) {
            None => Ok(Async::Ready(None)),
            Some(item) => {
                self.peeked = Some(item);
                Ok(Async::Ready(self.peeked.as_ref()))
            }
        }
    }
}
