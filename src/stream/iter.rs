use std::sync::Arc;
use std::marker;

use {Tokens, Wake};
use stream::{Stream, StreamResult};
use util;

pub struct IterStream<I, E> {
    iter: I,
    _marker: marker::PhantomData<fn() -> E>,
}

pub fn iter<I, E>(i: I) -> IterStream<I, E>
    where I: Iterator,
          I: Send + 'static,
          I::Item: Send + 'static,
          E: Send + 'static,
{
    IterStream {
        iter: i,
        _marker: marker::PhantomData,
    }
}

// TODO: what about iterators of results?
impl<I, E> Stream for IterStream<I, E>
    where I: Iterator,
          I: Send + 'static,
          I::Item: Send + 'static,
          E: Send + 'static,
{
    type Item = I::Item;
    type Error = E;

    fn poll(&mut self, _tokens: &Tokens)
            -> Option<StreamResult<I::Item, E>> {
        Some(Ok(self.iter.next()))
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        util::done(wake)
    }
}
