use std::sync::Arc;

use {Wake, Tokens, IntoFuture, Future, PollError};
use stream::{Stream, StreamResult};
use util;

pub struct Then<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: Option<F>,
}

pub fn new<S, F, U>(s: S, f: F) -> Then<S, F, U>
    where S: Stream,
          F: FnMut(Result<S::Item, S::Error>) -> U + Send + 'static,
          U: IntoFuture,
{
    Then {
        stream: s,
        future: None,
        f: Some(f),
    }
}

impl<S, F, U> Stream for Then<S, F, U>
    where S: Stream,
          F: FnMut(Result<S::Item, S::Error>) -> U + Send + 'static,
          U: IntoFuture,
{
    type Item = U::Item;
    type Error = U::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<U::Item, U::Error>> {
        if self.future.is_none() {
            let item = match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => Ok(e),
                Some(Ok(None)) => return Some(Ok(None)),
                Some(Err(PollError::Other(e))) => Err(e),
                Some(Err(PollError::Panicked(e))) => {
                    return Some(Err(PollError::Panicked(e)))
                }
                None => return None,
            };
            let mut f = match util::opt2poll(self.f.take()) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            match util::recover(|| (f(item).into_future(), f)) {
                Ok((future, f)) => {
                    self.future = Some(future);
                    self.f = Some(f);
                }
                Err(e) => return Some(Err(e)),
            }
        }
        assert!(self.future.is_some());
        let res = self.future.as_mut().unwrap().poll(tokens);
        if res.is_some() {
            self.future = None;
        }
        res.map(|r| r.map(Some))
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        match self.future {
            Some(ref mut s) => s.schedule(wake),
            None => self.stream.schedule(wake),
        }
    }
}
