use std::sync::Arc;

use {Wake, Tokens, PollError};
use stream::{Stream, StreamResult};
use util;

pub struct MapErr<S, F> {
    stream: S,
    f: Option<F>,
}

pub fn new<S, F, U>(s: S, f: F) -> MapErr<S, F>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    MapErr {
        stream: s,
        f: Some(f),
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
        let item = match self.stream.poll(tokens) {
            Some(Ok(e)) => return Some(Ok(e)),
            Some(Err(PollError::Other(e))) => e,
            Some(Err(PollError::Panicked(e))) => {
                return Some(Err(PollError::Panicked(e)))
            }
            None => return None,
        };
        let mut f = match util::opt2poll(self.f.take()) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };
        match util::recover(move || (f(item), f)) {
            Ok((e, f)) => {
                self.f = Some(f);
                Some(Err(PollError::Other(e)))
            }
            Err(e) => Some(Err(e))
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
