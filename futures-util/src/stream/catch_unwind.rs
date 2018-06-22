use std::prelude::v1::*;
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};
use std::mem::PinMut;

use futures_core::{Poll, Stream};
use futures_core::task;

/// Stream for the `catch_unwind` combinator.
///
/// This is created by the `Stream::catch_unwind` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct CatchUnwind<S> where S: Stream {
    stream: S,
    caught_unwind: bool,
}

pub fn new<S>(stream: S) -> CatchUnwind<S>
    where S: Stream + UnwindSafe,
{
    CatchUnwind { stream, caught_unwind: false }
}

impl<S: Stream> CatchUnwind<S> {
    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(caught_unwind -> bool);
}

impl<S> Stream for CatchUnwind<S>
    where S: Stream + UnwindSafe,
{
    type Item = Result<S::Item, Box<Any + Send>>;

    fn poll_next(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        if *self.caught_unwind() {
            return Poll::Ready(None)
        } else {
            let res = catch_unwind(AssertUnwindSafe(|| self.stream().poll_next(cx)));
            match res {
                Ok(poll) => poll.map(|opt| opt.map(Ok)),
                Err(e) => {
                    *self.caught_unwind() = true;
                    Poll::Ready(Some(Err(e)))
                },
            }
        }
    }
}
