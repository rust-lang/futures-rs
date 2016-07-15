use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};

/// A stream combinator which will change the type of a stream from one
/// type to another.
///
/// This is produced by the `Stream::map` method.
pub struct Map<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> Map<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    Map {
        stream: s,
        f: f,
    }
}

impl<S, F, U> Stream for Map<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<U, S::Error>> {
        match self.stream.poll(tokens) {
            Some(Ok(Some(e))) => Some(Ok(Some((self.f)(e)))),
            Some(Ok(None)) => Some(Ok(None)),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
