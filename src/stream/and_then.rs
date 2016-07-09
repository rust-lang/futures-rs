use std::sync::Arc;

use {Wake, Tokens, IntoFuture, Future};
use stream::{Stream, StreamResult};
use util;

pub struct AndThen<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: Option<F>,
}

pub fn new<S, F, U>(s: S, f: F) -> AndThen<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: IntoFuture<Error=S::Error>,
{
    AndThen {
        stream: s,
        future: None,
        f: Some(f),
    }
}

impl<S, F, U> Stream for AndThen<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: IntoFuture<Error=S::Error>,
{
    type Item = U::Item;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<U::Item, S::Error>> {
        if self.future.is_none() {
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
            match util::recover(|| (f(item).into_future(), f)) {
                Ok((future, f)) => {
                    self.future = Some(future);
                    self.f = Some(f);
                }
                Err(e) => return Some(Err(e)),
            }
        }

        assert!(self.future.is_some());
        // TODO: Tokens::all() if we just created the future?
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
