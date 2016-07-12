use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};

pub struct Filter<S, F> {
    stream: S,
    f: F,
}

pub fn new<S, F>(s: S, f: F) -> Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool + Send + 'static,
{
    Filter {
        stream: s,
        f: f,
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
            match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => {
                    if (self.f)(&e) {
                        return Some(Ok(Some(e)))
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
