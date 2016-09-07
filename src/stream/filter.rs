use {Async, Poll};
use stream::Stream;

/// A stream combinator used to filter the results of a stream and only yield
/// some values.
///
/// This structure is produced by the `Stream::filter` method.
#[must_use = "streams do nothing unless polled"]
pub struct Filter<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F>(s: S, f: F) -> Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool,
{
    Filter {
        stream: s,
        f: f,
    }
}

impl<S, F> Stream for Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        loop {
            match try_ready!(self.stream.poll()) {
                Some(e) => {
                    if (self.f)(&e) {
                        return Ok(Async::Ready(Some(e)))
                    }
                }
                None => return Ok(Async::Ready(None)),
            }
        }
    }
}
