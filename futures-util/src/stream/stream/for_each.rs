use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`for_each`](super::StreamExt::for_each) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ForEach<St, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        #[pin]
        future: Option<Fut>,
        completed: usize,
    }
}

impl<St, Fut, F> fmt::Debug for ForEach<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ForEach")
            .field("stream", &self.stream)
            .field("future", &self.future)
            .field("completed", &self.completed)
            .finish()
    }
}

impl<St, Fut, F> ForEach<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, future: None, completed: 0 }
    }
}

impl<St, Fut, F> FusedFuture for ForEach<St, Fut, F>
where
    St: FusedStream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.future.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Future for ForEach<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future<Output = ()>,
{
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<usize> {
        let mut this = self.project();
        loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                ready!(fut.poll(cx));
                this.future.set(None);
                // On overflow the returned count serves as a lower bound
                // for the actual number of completed futures.
                *this.completed = this.completed.saturating_add(1);
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                this.future.set(Some((this.f)(item)));
            } else {
                break;
            }
        }
        Poll::Ready(*this.completed)
    }
}
