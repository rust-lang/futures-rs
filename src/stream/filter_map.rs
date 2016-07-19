use std::sync::Arc;

use {Wake, Tokens, Poll};
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
          F: FnMut(S::Item) -> Option<B> + Send + 'static,
{
    FilterMap {
        stream: s,
        f: f,
    }
}

impl<S, F, B> Stream for FilterMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Option<B> + Send + 'static,
          B: Send + 'static,
{
    type Item = B;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Poll<Option<B>, S::Error> {
        loop {
            match try_poll!(self.stream.poll(tokens)) {
                Ok(Some(e)) => {
                    if let Some(e) = (self.f)(e) {
                        return Poll::Ok(Some(e))
                    }
                }
                Ok(None) => return Poll::Ok(None),
                Err(e) => return Poll::Err(e),
            }
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
