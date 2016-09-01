use {Async, Poll};
use stream::Stream;

/// A combinator used to filter the results of a stream and simultaneously map
/// them to a different type.
///
/// This structure is returned by the `Stream::filter_map` method.
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

impl<S, F, B> Stream for FilterMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Option<B>,
{
    type Item = B;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<B>, S::Error> {
        loop {
            match try_ready!(self.stream.poll()) {
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
