use core::{fmt, pin::Pin};

use futures_core::{
    future::{FusedFuture, Future},
    ready,
    stream::TryStream,
    task::{Context, Poll},
};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_find`](super::TryStreamExt::try_find) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryFind<St, Fut, F>
    where
        St: TryStream,
    {
        #[pin]
        stream: St,
        f: F,
        done: bool,
        pending_item: Option<St::Ok>,
        #[pin]
        pending_fut: Option<Fut>,
    }
}

impl<St, Fut, F> fmt::Debug for TryFind<St, Fut, F>
where
    St: TryStream + fmt::Debug,
    St::Ok: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryFind")
            .field("stream", &self.stream)
            .field("done", &self.done)
            .field("pending_item", &self.pending_item)
            .field("pending_fut", &self.pending_fut)
            .finish()
    }
}

impl<St, Fut, F> TryFind<St, Fut, F>
where
    St: TryStream,
    F: FnMut(&St::Ok) -> Fut,
    Fut: Future<Output = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, done: false, pending_item: None, pending_fut: None }
    }
}

impl<St, Fut, F> FusedFuture for TryFind<St, Fut, F>
where
    St: TryStream,
    F: FnMut(&St::Ok) -> Fut,
    Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.done && self.pending_fut.is_none()
    }
}

impl<St, Fut, F> Future for TryFind<St, Fut, F>
where
    St: TryStream,
    F: FnMut(&St::Ok) -> Fut,
    Fut: Future<Output = bool>,
{
    type Output = Result<Option<St::Ok>, St::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.pending_fut.as_mut().as_pin_mut() {
                let matched = ready!(fut.poll(cx));
                this.pending_fut.set(None);
                if matched {
                    *this.done = true;
                    break Ok(this.pending_item.take());
                }
                *this.pending_item = None;
            } else if !*this.done {
                match ready!(this.stream.as_mut().try_poll_next(cx)) {
                    Some(Ok(item)) => {
                        this.pending_fut.set(Some((this.f)(&item)));
                        *this.pending_item = Some(item);
                    }
                    Some(Err(err)) => {
                        *this.done = true;
                        break Err(err);
                    }
                    None => {
                        *this.done = true;
                        break Ok(None);
                    }
                }
            } else {
                panic!("TryFind polled after completion")
            }
        })
    }
}
