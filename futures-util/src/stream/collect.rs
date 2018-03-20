use std::prelude::v1::*;

use std::mem;

use futures_core::{Future, Poll, Async, Stream};
use futures_core::task;

/// A future which collects all of the values of a stream into a vector.
///
/// This future is created by the `Stream::collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Collect<S, C> where S: Stream {
    stream: S,
    items: C,
}

pub fn new<S, C>(s: S) -> Collect<S, C>
    where S: Stream, C: Default
{
    Collect {
        stream: s,
        items: Default::default(),
    }
}

impl<S: Stream, C: Default> Collect<S, C> {
    fn finish(&mut self) -> C {
        mem::replace(&mut self.items, Default::default())
    }
}

impl<S, C> Future for Collect<S, C>
    where S: Stream, C: Default + Extend<S:: Item>
{
    type Item = C;
    type Error = S::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<C, S::Error> {
        loop {
            match self.stream.poll_next(cx) {
                Ok(Async::Ready(Some(e))) => self.items.extend(Some(e)),
                Ok(Async::Ready(None)) => return Ok(Async::Ready(self.finish())),
                Ok(Async::Pending) => return Ok(Async::Pending),
                Err(e) => {
                    self.finish();
                    return Err(e)
                }
            }
        }
    }
}
