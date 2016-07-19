use std::mem;
use std::sync::Arc;

use {Wake, Tokens, Future, TOKENS_ALL, Poll};
use stream::Stream;

/// A future which collects all of the values of a stream into a vector.
///
/// This future is created by the `Stream::collect` method.
pub struct Collect<S> where S: Stream {
    stream: S,
    items: Vec<S::Item>,
}

pub fn new<S>(s: S) -> Collect<S>
    where S: Stream,
{
    Collect {
        stream: s,
        items: Vec::new(),
    }
}

impl<S: Stream> Collect<S> {
    fn finish(&mut self) -> Vec<S::Item> {
        mem::replace(&mut self.items, Vec::new())
    }
}

impl<S> Future for Collect<S>
    where S: Stream,
{
    type Item = Vec<S::Item>;
    type Error = S::Error;

    fn poll(&mut self, mut tokens: &Tokens) -> Poll<Vec<S::Item>, S::Error> {
        loop {
            match try_poll!(self.stream.poll(tokens)) {
                Ok(Some(e)) => self.items.push(e),
                Ok(None) => return Poll::Ok(self.finish()),
                Err(e) => {
                    self.finish();
                    return Poll::Err(e)
                }
            }
            tokens = &TOKENS_ALL;
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
