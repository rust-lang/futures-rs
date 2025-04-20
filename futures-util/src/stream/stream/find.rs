use crate::fns::FnMut1;
use crate::Stream;

use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::task::{ready, Context, Poll};
use futures_core::FusedFuture;
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`find`](super::StreamExt::find) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Find<St, Fut, F>
        where St: Stream,
    {
        #[pin]
        stream: St,
        f: F,
        done: bool,
        #[pin]
        pending_fut: Option<Fut>,
        pending_item: Option<St::Item>,
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
            .field("pending_fut", &self.pending_fut)
            .field("pending_item", &self.pending_item)
            .finish()
    }
}

impl<St, Fut, F> Find<St, Fut, F>
where
    St: Stream,
    F: for<'a> FnMut1<&'a St::Item, Output = Fut>,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, done: false, pending_fut: None, pending_item: None }
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
    St: futures_core::Stream,
    F: for<'a> FnMut1<&'a St::Item, Output = Fut>,
    Fut: Future<Output = bool>,
{
    type Output = Option<St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        let mut this = self.project();
        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                // we're currently processing a future to produce a new value
                let res = ready!(fut.poll(cx));
                this.pending_fut.set(None);
                if res {
                    *this.done = true;
                    break this.pending_item.take();
                }
            } else if !*this.done {
                // we're waiting on a new item from the stream
                match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => {
                        this.pending_fut.set(Some(this.f.call_mut(&item)));
                        *this.pending_item = Some(item);
                    }
                    None => {
                        *this.done = true;
                        break None;
                    }
                }
            } else {
                break None;
            }
        })
    }
}
