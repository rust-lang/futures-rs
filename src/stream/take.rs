use {Async, Poll};
use stream::Stream;
use task::Task;

/// A stream combinator which returns a maximum number of elements.
///
/// This structure is produced by the `Stream::take` method.
#[must_use = "streams do nothing unless polled"]
pub struct Take<S> {
    stream: S,
    remaining: u64,
}

pub fn new<S>(s: S, amt: u64) -> Take<S>
    where S: Stream,
{
    Take {
        stream: s,
        remaining: amt,
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S> ::sink::Sink for Take<S>
    where S: ::sink::Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    fn start_send(&mut self, task: &Task, item: S::SinkItem) -> ::StartSend<S::SinkItem, S::SinkError> {
        self.stream.start_send(task, item)
    }

    fn poll_complete(&mut self, task: &Task) -> Poll<(), S::SinkError> {
        self.stream.poll_complete(task)
    }
}

impl<S> Stream for Take<S>
    where S: Stream,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, task: &Task) -> Poll<Option<S::Item>, S::Error> {
        if self.remaining == 0 {
            Ok(Async::Ready(None))
        } else {
            let next = try_ready!(self.stream.poll(task));
            match next {
                Some(_) => self.remaining -= 1,
                None => self.remaining = 0,
            }
            Ok(Async::Ready(next))
        }
    }
}
