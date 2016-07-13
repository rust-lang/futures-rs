use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};

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

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<S::Item, U>> {
        match self.stream.poll(tokens) {
            Some(Ok(e)) => Some(Ok(e)),
            Some(Err(e)) => Some(Err((self.f)(e))),
            None => None,
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
