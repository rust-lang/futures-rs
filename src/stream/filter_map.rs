use {Async, Poll};
use stream::Stream;
use task::Task;

/// A combinator used to filter the results of a stream and simultaneously map
/// them to a different type.
///
/// This structure is returned by the `Stream::filter_map` method.
#[must_use = "streams do nothing unless polled"]
pub struct FilterMap<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F, B>(s: S, f: F) -> FilterMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Option<B>,
{
    FilterMap {
        stream: s,
        f: f,
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S, F> ::sink::Sink for FilterMap<S, F>
    where S: ::sink::Sink
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

impl<S, F, B> Stream for FilterMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Option<B>,
{
    type Item = B;
    type Error = S::Error;

    fn poll(&mut self, task: &Task) -> Poll<Option<B>, S::Error> {
        loop {
            match try_ready!(self.stream.poll(task)) {
                Some(e) => {
                    if let Some(e) = (self.f)(e) {
                        return Ok(Async::Ready(Some(e)))
                    }
                }
                None => return Ok(Async::Ready(None)),
            }
        }
    }
}
