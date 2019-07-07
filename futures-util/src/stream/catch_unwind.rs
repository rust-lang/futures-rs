use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, unsafe_project};
use std::any::Any;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};
use std::pin::Pin;

/// Stream for the [`catch_unwind`](super::StreamExt::catch_unwind) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct CatchUnwind<St: Stream> {
    #[pin]
    stream: St,
    caught_unwind: bool,
}

impl<St: Stream + UnwindSafe> CatchUnwind<St> {
    pub(super) fn new(stream: St) -> CatchUnwind<St> {
        CatchUnwind { stream, caught_unwind: false }
    }
}

impl<St: Stream + UnwindSafe> Stream for CatchUnwind<St>
{
    type Item = Result<St::Item, Box<dyn Any + Send>>;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if *self.caught_unwind {
            Poll::Ready(None)
        } else {
            let res = catch_unwind(AssertUnwindSafe(|| {
                self.stream.as_mut().poll_next(cx)
            }));

            match res {
                Ok(poll) => poll.map(|opt| opt.map(Ok)),
                Err(e) => {
                    *self.caught_unwind = true;
                    Poll::Ready(Some(Err(e)))
                },
            }
        }
    }
}
