use std::sync::Arc;

use {Wake, Tokens, Poll};
use stream::Stream;

/// A stream combinator which will change the error type of a stream from one
/// type to another.
///
/// This is produced by the `Stream::map_err` method.
pub struct MapErr<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> MapErr<S, F>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    MapErr {
        stream: s,
        f: f,
    }
}

impl<S, F, U> Stream for MapErr<S, F>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = S::Item;
    type Error = U;

    fn poll(&mut self, tokens: &Tokens) -> Poll<Option<S::Item>, U> {
        self.stream.poll(tokens).map_err(&mut self.f)
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
