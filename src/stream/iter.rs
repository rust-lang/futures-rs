use std::sync::Arc;
use std::marker;

use {Tokens, Wake};
use stream::{Stream, StreamResult};
use util;

/// A stream which is just a shim over an underlying instance of `Iterator`.
///
/// This stream will never block and is always ready.
pub struct IterStream<I, E> {
    iter: I,
    _marker: marker::PhantomData<fn() -> E>,
}

/// Converts an `Iterator` into a `Stream` which is always ready to yield the
/// next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter simply
/// always calls `iter.next()` and returns that. Additionally, the error type is
/// generic here as it will never be returned, instead the type of the iterator
/// will always be returned upwards as a successful value.
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

    fn schedule(&mut self, wake: &Arc<Wake>) {
        util::done(wake)
    }
}
