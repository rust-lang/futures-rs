use core::mem::PinMut;
use core::marker::Unpin;

use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A future of the next element of a stream.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Next<'a, S: 'a> {
    stream: &'a mut S,
}

pub fn new<'a, S: Stream + Unpin>(stream: &'a mut S) -> Next<'a, S> {
    Next { stream }
}

impl<'a, S: Stream + Unpin> Future for Next<'a, S> {
    type Output = Option<S::Item>;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        PinMut::new(&mut *self.stream).poll_next(cx)
    }
}
