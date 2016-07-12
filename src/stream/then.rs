use std::sync::Arc;

use {Wake, Tokens, IntoFuture, Future, ALL_TOKENS};
use stream::{Stream, StreamResult};

pub struct Then<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> Then<S, F, U>
    where S: Stream,
          F: FnMut(Result<S::Item, S::Error>) -> U + Send + 'static,
          U: IntoFuture,
{
    Then {
        stream: s,
        future: None,
        f: f,
    }
}

impl<S, F, U> Stream for Then<S, F, U>
    where S: Stream,
          F: FnMut(Result<S::Item, S::Error>) -> U + Send + 'static,
          U: IntoFuture,
{
    type Item = U::Item;
    type Error = U::Error;

    fn poll(&mut self, mut tokens: &Tokens)
            -> Option<StreamResult<U::Item, U::Error>> {
        if self.future.is_none() {
            let item = match self.stream.poll(tokens) {
                Some(Ok(None)) => return Some(Ok(None)),
                Some(Ok(Some(e))) => Ok(e),
                Some(Err(e)) => Err(e),
                None => return None,
            };
            self.future = Some((self.f)(item).into_future());
            tokens = &ALL_TOKENS;
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
