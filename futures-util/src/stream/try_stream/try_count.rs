use core::fmt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_count`](super::TryStreamExt::try_count) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryCount<St> {
        #[pin]
        stream: St,
        count: usize
    }
}

impl<St> fmt::Debug for TryCount<St>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryCount")
            .field("stream", &self.stream)
            .field("count", &self.count)
            .finish()
    }
}

impl<St> TryCount<St>
where St: TryStream,
{
    pub(super) fn new(stream: St) -> Self {
        Self {
            stream,
            count: 0
        }
    }
}

impl<St, T, E> Future for TryCount<St>
    where St: TryStream<Ok = T, Error = E>,
{
    type Output = Result<usize, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        Poll::Ready(loop {
            match ready!(this.stream.as_mut().try_poll_next(cx)) {
                Some(Err(err)) => break Err(err),
                Some(Ok(_)) => {
                    *this.count += 1;
                },
                None => break Ok(*this.count)
            }
        })
    }
}
