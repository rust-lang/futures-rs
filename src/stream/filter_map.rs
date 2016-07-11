use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};
use util;

pub struct FilterMap<S, F> {
    stream: S,
    f: Option<F>,
}

pub fn new<S, F, B>(s: S, f: F) -> FilterMap<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> Option<B> + Send + 'static,
{
    FilterMap {
        stream: s,
        f: Some(f),
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
            let item = match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => e,
                Some(Ok(None)) => return Some(Ok(None)),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };
            let mut f = match util::opt2poll(self.f.take()) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            match util::recover(move || (f(item), f)) {
                Ok((None, f)) => self.f = Some(f),
                Ok((Some(item), f)) => {
                    self.f = Some(f);
                    return Some(Ok(Some(item)))
                }
                Err(e) => return Some(Err(e))
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
