use {Async, Poll};
use stream::Stream;

/// A stream combinator which returns a maximum number of elements.
///
/// This structure is produced by the `Stream::take` method.
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

impl<S> Stream for Take<S>
    where S: Stream,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        if self.remaining == 0 {
            Ok(Async::Ready(None))
        } else {
            match self.stream.poll() {
                e @ Ok(Async::Ready(Some(_))) => {
                    self.remaining -= 1;
                    e
                }
                other => other,
            }
        }
    }
}
