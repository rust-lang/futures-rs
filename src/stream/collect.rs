use std::mem;
use std::sync::Arc;

use {Wake, Tokens, Future, TOKENS_ALL};
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

    fn poll(&mut self, mut tokens: &Tokens)
            -> Option<Result<Vec<S::Item>, S::Error>> {
        loop {
            match self.stream.poll(tokens) {
                Some(Ok(Some(e))) => self.items.push(e),
                Some(Ok(None)) => return Some(Ok(self.finish())),
                Some(Err(e)) => {
                    self.finish();
                    return Some(Err(e))
                }
                None => return None,
            }
            tokens = &TOKENS_ALL;
        }
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        self.stream.schedule(wake)
    }
}
