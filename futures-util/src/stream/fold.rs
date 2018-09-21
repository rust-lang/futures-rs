use core::marker::Unpin;
use core::pin::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{self, Poll};
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

impl<St, Fut, T, F> Future for Fold<St, Fut, T, F>
    where St: Stream,
          F: FnMut(T, St::Item) -> Fut,
          Fut: Future<Output = T>,
{
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<T> {
        loop {
            // we're currently processing a future to produce a new accum value
            if self.accum().is_none() {
                let accum = ready!(self.future().as_pin_mut().unwrap().poll(cx));
                *self.accum() = Some(accum);
                PinMut::set(self.future(), None);
            }

            let item = ready!(self.stream().poll_next(cx));
            let accum = self.accum().take()
                .expect("Fold polled after completion");

            if let Some(e) = item {
                let future = (self.f())(accum, e);
                PinMut::set(self.future(), Some(future));
            } else {
                return Poll::Ready(accum)
            }
        }
    }
}
