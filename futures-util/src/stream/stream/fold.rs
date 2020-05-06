use core::fmt;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

/// Future for the [`fold`](super::StreamExt::fold) method.
#[pin_project]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Fold<St, Fut, T, F> {
    #[pin]
    stream: St,
    f: F,
    accum: Option<T>,
    #[pin]
    future: Option<Fut>,
}

impl<St, Fut, T, F> fmt::Debug for Fold<St, Fut, T, F>
where
    St: fmt::Debug,
    Fut: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fold")
            .field("stream", &self.stream)
            .field("accum", &self.accum)
            .field("future", &self.future)
            .finish()
    }
}

impl<St, Fut, T, F> Fold<St, Fut, T, F>
where St: Stream,
      F: FnMut(T, St::Item) -> Fut,
      Fut: Future<Output = T>,
{
    pub(super) fn new(stream: St, f: F, t: T) -> Fold<St, Fut, T, F> {
        Fold {
            stream,
            f,
            accum: Some(t),
            future: None,
        }
    }
}

impl<St, Fut, T, F> FusedFuture for Fold<St, Fut, T, F>
    where St: Stream,
          F: FnMut(T, St::Item) -> Fut,
          Fut: Future<Output = T>,
{
    fn is_terminated(&self) -> bool {
        self.accum.is_none() && self.future.is_none()
    }
}

impl<St, Fut, T, F> Future for Fold<St, Fut, T, F>
    where St: Stream,
          F: FnMut(T, St::Item) -> Fut,
          Fut: Future<Output = T>,
{
    type Output = T;

    #[project]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        #[project]
        let Fold { mut stream, f, accum, mut future } = self.project();
        Poll::Ready(loop {
            if let Some(fut) = future.as_mut().as_pin_mut() {
                // we're currently processing a future to produce a new accum value
                *accum = Some(ready!(fut.poll(cx)));
                future.set(None);
            } else if accum.is_some() {
                // we're waiting on a new item from the stream
                let res = ready!(stream.as_mut().poll_next(cx));
                let a = accum.take().unwrap();
                if let Some(item) = res {
                    future.set(Some(f(a, item)));
                } else {
                    break a;
                }
            } else {
                panic!("Fold polled after completion")
            }
        })
    }
}
