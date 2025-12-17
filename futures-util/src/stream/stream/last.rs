use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`last`](super::StreamExt::last) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Last<St: Stream> {
        #[pin]
        stream: St,
        last: Option<St::Item>,
        done: bool,
    }
}

impl<St> fmt::Debug for Last<St>
where
    St: Stream + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Last").field("stream", &self.stream).finish()
    }
}

impl<St> Last<St>
where
    St: Stream,
{
    pub(super) fn new(stream: St) -> Self {
        Self { stream, last: None, done: false }
    }
}

impl<St> FusedFuture for Last<St>
where
    St: FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.done
    }
}

impl<St> Future for Last<St>
where
    St: Stream,
{
    type Output = Option<St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if *this.done {
            panic!("Last polled after completion");
        }

        Poll::Ready(loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(item) => *this.last = Some(item),
                None => {
                    *this.done = true;
                    break this.last.take();
                }
            }
        })
    }
}
