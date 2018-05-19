use std::prelude::v1::*;

use std::mem::{self, PinMut};

use futures_core::{Future, Poll, Stream};
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
    fn finish(mut self: PinMut<Self>) -> C {
        mem::replace(self.items(), Default::default())
    }

    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(items -> C);
}

impl<S, C> Future for Collect<S, C>
    where S: Stream, C: Default + Extend<S:: Item>
{
    type Output = C;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<C> {
        loop {
            match try_ready!(self.stream().poll_next(cx)) {
                Some(e) => self.items().extend(Some(e)),
                None => return Poll::Ready(self.finish()),
            }
        }
    }
}
