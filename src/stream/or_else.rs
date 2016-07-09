use std::sync::Arc;

use {Wake, Tokens, IntoFuture, Future, PollError};
use stream::{Stream, StreamResult};
use util;

pub struct OrElse<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: Option<F>,
}

pub fn new<S, F, U>(s: S, f: F) -> OrElse<S, F, U>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: IntoFuture<Item=S::Item>,
{
    OrElse {
        stream: s,
        future: None,
        f: Some(f),
    }
}

impl<S, F, U> Stream for OrElse<S, F, U>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: IntoFuture<Item=S::Item>,
{
    type Item = S::Item;
    type Error = U::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<S::Item, U::Error>> {
        if self.future.is_none() {
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


