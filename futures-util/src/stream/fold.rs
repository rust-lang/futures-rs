use core::mem::PinMut;

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

impl<S, Fut, T, F> Future for Fold<S, Fut, T, F>
    where S: Stream,
          F: FnMut(T, S::Item) -> Fut,
          Fut: Future<Output = T>,
{
    type Output = T;

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<T> {
        loop {
            if self.accum().is_some() {
                let item = try_ready!(self.stream().poll_next(cx));
                let accum = self.accum().take().unwrap();

                if let Some(e) = item {
                    let fut = (self.f())(accum, e);
                    self.fut().assign(Some(fut));
                } else {
                    return Poll::Ready(accum)
                }
            } else {
                let accum = try_ready!(self.fut().as_pin_mut().unwrap().poll(cx));
                *self.accum() = Some(accum);
                self.fut().assign(None);
            }
        }
    }
}
