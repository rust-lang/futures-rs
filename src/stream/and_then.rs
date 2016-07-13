use std::sync::Arc;

use {Wake, Tokens, IntoFuture, Future, ALL_TOKENS};
use stream::{Stream, StreamResult};

/// A stream combinator which chains a computation onto values produced by a
/// stream.
///
/// This structure is produced by the `Stream::and_then` method.
pub struct AndThen<S, F, U>
    where U: IntoFuture,
{
    stream: S,
    future: Option<U::Future>,
    f: F,
}

pub fn new<S, F, U>(s: S, f: F) -> AndThen<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: IntoFuture<Error=S::Error>,
{
    AndThen {
        stream: s,
        future: None,
        f: f,
    }
}

impl<S, F, U> Stream for AndThen<S, F, U>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: IntoFuture<Error=S::Error>,
{
    type Item = U::Item;
    type Error = S::Error;

    fn poll(&mut self, mut tokens: &Tokens)
            -> Option<StreamResult<U::Item, S::Error>> {
        if self.future.is_none() {
            let item = match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => e,
                Some(Ok(None)) => return Some(Ok(None)),
                Some(Err(e)) => return Some(Err(e)),
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
