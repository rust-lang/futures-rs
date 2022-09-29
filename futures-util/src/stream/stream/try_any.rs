use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`try_any`](super::TryStreamExt::try_any) method.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryAny<St, Fut, F> {
        #[pin]
        stream: St,
        f: F,
        accum: Option<bool>,
        #[pin]
        future: Option<Fut>
    }
}

impl<St, Fut, F> fmt::Debug for TryAny<St, Fut, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryAny")
            .field("stream", &self.stream)
            .field("accum", &self.accum)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, F> TryAny<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: TryFuture<Ok = bool>,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f, accum: Some(false), future: None }
    }
}

impl<St, Fut, F> FusedFuture for TryAny<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: TryFuture<Ok = bool>,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.future.is_none()
    }
}

impl<St, Fut, F> Future for TryAny<St, Fut, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: TryFuture<Ok = bool>,
{
    type Output = Result<bool, Fut::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        Poll::Ready(loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                match ready!(fut.try_poll(cx)) {
                    Ok(a) => {
                        let acc = this.accum.unwrap() || a;
                        if acc {
                            break Ok(true);
                        }
                        this.future.set(None);
                    }
                    Err(e) => break Err(e),
                }
            } else if this.accum.is_some() {
                match ready!(this.stream.as_mut().poll_next(cx)) {
                    Some(item) => this.future.set(Some((this.f)(item))),
                    None => break Ok(this.accum.take().unwrap()),
                }
            } else {
                panic!("All polled after completion")
            }
        })
    }
}
