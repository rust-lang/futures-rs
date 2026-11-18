use core::{fmt, pin::Pin};

use futures_core::{
    future::{FusedFuture, Future},
    ready,
    stream::Stream,
    task::{Context, Poll},
};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`find`](super::StreamExt::find) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Find<St, Fut, F>
    where
        St: Stream,
    {
        #[pin]
        stream: St,
        f: F,
        done: bool,
        pending_item: Option<St::Item>,
        #[pin]
        pending_fut: Option<Fut>,
    }
}

impl<St, Fut, F> fmt::Debug for Find<St, Fut, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Find")
            .field("stream", &self.stream)
            .field("done", &self.done)
            .field("pending_item", &self.pending_item)
            .field("pending_fut", &self.pending_fut)
            .finish()
    }
}

impl<St, Fut, F> Find<St, Fut, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, done: false, pending_item: None, pending_fut: None }
    }
}

impl<St, Fut, F> FusedFuture for Find<St, Fut, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.done && self.pending_fut.is_none()
    }
}

impl<St, Fut, F> Future for Find<St, Fut, F>
where
    St: Stream,
    F: FnMut(&St::Item) -> Fut,
    Fut: Future<Output = bool>,
{
    type Output = Option<St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let matched = ready!(fut.poll(cx));
                this.pending_fut.set(None);
                if matched {
                    *this.done = true;
                    break this.pending_item.take();
                }
                *this.pending_item = None;
            } else if !*this.done {
                match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => {
                        this.pending_fut.set(Some((this.f)(&item)));
                        *this.pending_item = Some(item);
                    }
                    None => {
                        *this.done = true;
                        break None;
                    }
                }
            } else {
                panic!("Find polled after completion")
            }
        })
    }
}
