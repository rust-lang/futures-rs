use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{LocalWaker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A future used to collect all the results of a stream into one generic type.
///
/// This future is returned by the `Stream::fold` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Fold<St, Fut, T, F> {
    stream: St,
    f: F,
    accum: Option<T>,
    future: Option<Fut>,
}

impl<St: Unpin, Fut: Unpin, T, F> Unpin for Fold<St, Fut, T, F> {}

impl<St, Fut, T, F> Fold<St, Fut, T, F>
where St: Stream,
      F: FnMut(T, St::Item) -> Fut,
      Fut: Future<Output = T>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_unpinned!(accum: Option<T>);
    unsafe_pinned!(future: Option<Fut>);

    pub(super) fn new(stream: St, f: F, t: T) -> Fold<St, Fut, T, F> {
        Fold {
            stream,
            f,
            accum: Some(t),
            future: None,
        }
    }
}

impl<St, Fut, T, F> FusedFuture for Fold<St, Fut, T, F> {
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

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<T> {
        loop {
            // we're currently processing a future to produce a new accum value
            if self.as_mut().accum().is_none() {
                let accum = ready!(self.as_mut().future().as_pin_mut().unwrap().poll(lw));
                *self.as_mut().accum() = Some(accum);
                self.as_mut().future().set(None);
            }

            let item = ready!(self.as_mut().stream().poll_next(lw));
            let accum = self.as_mut().accum().take()
                .expect("Fold polled after completion");

            if let Some(e) = item {
                let future = (self.as_mut().f())(accum, e);
                self.as_mut().future().set(Some(future));
            } else {
                return Poll::Ready(accum)
            }
        }
    }
}
