use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
use std::any::Any;
use std::mem::PinMut;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};
use std::prelude::v1::*;

/// Stream for the `catch_unwind` combinator.
///
/// This is created by the `Stream::catch_unwind` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct CatchUnwind<St: Stream> {
    stream: St,
    caught_unwind: bool,
}

impl<St: Stream + UnwindSafe> CatchUnwind<St> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(caught_unwind: bool);

    pub(super) fn new(stream: St) -> CatchUnwind<St> {
        CatchUnwind { stream, caught_unwind: false }
    }
}

impl<St: Stream + UnwindSafe> Stream for CatchUnwind<St>
{
    type Item = Result<St::Item, Box<dyn Any + Send>>;

    fn poll_next(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Option<Self::Item>> {
        if *self.caught_unwind() {
            Poll::Ready(None)
        } else {
            let res = catch_unwind(AssertUnwindSafe(|| {
                self.stream().poll_next(cx)
            }));

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
