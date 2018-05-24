use core::mem::PinMut;
use core::marker::Unpin;

use {PinMutExt, OptionExt};

use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A future used to collect all the results of a stream into one generic type.
///
/// This future is returned by the `Stream::fold` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Fold<S, Fut, T, F> {
    stream: S,
    f: F,
    accum: Option<T>,
    fut: Option<Fut>,
}

pub fn new<S, Fut, T, F>(s: S, f: F, t: T) -> Fold<S, Fut, T, F>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut,
          Fut: Future<Output = T>,
{
    Fold {
        stream: s,
        f: f,
        accum: Some(t),
        fut: None,
    }
}

impl<S, Fut, T, F> Fold<S, Fut, T, F> {
    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(f -> F);
    unsafe_unpinned!(accum -> Option<T>);
    unsafe_pinned!(fut -> Option<Fut>);
}

unsafe impl<S: Unpin, Fut: Unpin, T, F> Unpin for Fold<S, Fut, T, F> {}

impl<S, Fut, T, F> Future for Fold<S, Fut, T, F>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut,
          Fut: Future<Output = T>,
{
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<T> {
        loop {
            // we're currently processing a future to produce a new accum value
            if self.accum().is_none() {
                let accum = ready!(self.fut().as_pin_mut().unwrap().poll(cx));
                *self.accum() = Some(accum);
                self.fut().assign(None);
            }

            let item = ready!(self.stream().poll_next(cx));
            let accum = self.accum().take()
                .expect("Fold polled after completion");

            if let Some(e) = item {
                let fut = (self.f())(accum, e);
                self.fut().assign(Some(fut));
            } else {
                return Poll::Ready(accum)
            }
        }
    }
}
