use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};
use util;

pub struct Map<S, F> {
    stream: S,
    f: Option<F>,
}

pub fn new<S, F, U>(s: S, f: F) -> Map<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    Map {
        stream: s,
        f: Some(f),
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
            Ok((e, f)) => {
                self.f = Some(f);
                Some(Ok(Some(e)))
            }
            Err(e) => Some(Err(e))
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
