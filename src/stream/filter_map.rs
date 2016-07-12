use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};

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

    fn poll(&mut self, tokens: &Tokens) -> Option<StreamResult<B, S::Error>> {
        loop {
            match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => {
                    match (self.f)(e) {
                        Some(e) => return Some(Ok(Some(e))),
                        None => {}
                    }
                }
                Some(Ok(None)) => return Some(Ok(None)),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
