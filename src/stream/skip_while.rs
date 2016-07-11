use std::sync::Arc;

use {Wake, Tokens, PollError};
use stream::{Stream, StreamResult};
use util;

pub struct SkipWhile<S, P> {
    stream: S,
    pred: Option<P>,
    done_skipping: bool,
}

pub fn new<S, P>(s: S, p: P) -> SkipWhile<S, P>
    where S: Stream,
          P: FnMut(&S::Item) -> Result<bool, S::Error> + Send + 'static
{
    SkipWhile {
        stream: s,
        pred: Some(p),
        done_skipping: false,
    }
}

impl<S, P> Stream for SkipWhile<S, P>
    where S: Stream,
          P: FnMut(&S::Item) -> Result<bool, S::Error> + Send + 'static
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens) -> Option<StreamResult<S::Item, S::Error>> {
        if self.done_skipping {
            return self.stream.poll(tokens);
        }

        loop {
            let item = match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => e,
                Some(Ok(None)) => return Some(Ok(None)),
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };
            let mut pred = match util::opt2poll(self.pred.take()) {
                Ok(pred) => pred,
                Err(e) => return Some(Err(e)),
            };
            match util::recover(move || (pred(&item), item, pred)) {
                Ok((res, item, pred)) => {
                    self.pred = Some(pred);
                    if let Ok(false) = res {
                        self.done_skipping = true;
                        return Some(Ok(Some(item)));
                    } else if let Err(e) = res {
                        return Some(Err(PollError::Other(e)));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        self.stream.schedule(wake)
    }
}

impl<S, P> SkipWhile<S, P> {
    pub fn into_inner(self) -> S {
        self.stream
    }
}
