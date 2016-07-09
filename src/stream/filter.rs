use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};
use util;

pub struct Filter<S, F> {
    stream: S,
    f: Option<F>,
}

pub fn new<S, F>(s: S, f: F) -> Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool + Send + 'static,
{
    Filter {
        stream: s,
        f: Some(f),
    }
}

impl<S, F> Stream for Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool + Send + 'static,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<S::Item, S::Error>> {
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
            match util::recover(move || (f(&item), item, f)) {
                Ok((false, _, f)) => self.f = Some(f),
                Ok((true, item, f)) => {
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
