use {Async, Poll};
use stream::Stream;
use task;

/// A stream combinator that resubmits the task after a certain number of elements
/// have been successfully polled, even if the stream would be ready for further polls.
///
/// This structure is produced by the `Stream::yield_after` method.
#[must_use = "streams do nothing unless polled"]
pub struct YieldAfter<S> {
    stream: S,
    yield_after: u64,
    remaining: u64,
}

pub fn new<S>(s: S, yield_after: u64) -> YieldAfter<S>
    where S: Stream,
{
    YieldAfter {
        stream: s,
        yield_after: yield_after,
        remaining: yield_after
    }

}

//TODO: What about the forwarding Sink? What should be done here?

impl<S> Stream for YieldAfter<S>
    where S: Stream
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        if self.remaining == 0 {
            self.remaining = self.yield_after;
            // Immediately reschedule the task. If the executor has fairness, than so
            // does this stream.
            task::park().unpark();
            Ok(Async::NotReady)
        } else {
            match try!(self.stream.poll()) {
                Async::Ready(next) => {
                    match next {
                        Some(_) => self.remaining -= 1,
                        None => self.remaining = 0,
                    }
                    Ok(Async::Ready(next))
                }
                Async::NotReady => {
                    self.remaining = self.yield_after;
                    Ok(Async::NotReady)
                }
            }
        }
    }
}