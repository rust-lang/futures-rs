use core::mem::PinMut;

use {PinMutExt, OptionExt};

use futures_core::{Future, Poll, Stream};
use futures_core::task;

/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::for_each` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEach<S, U, F> {
    stream: S,
    f: F,
    fut: Option<U>,
}

pub fn new<S, U, F>(s: S, f: F) -> ForEach<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: Future<Output = ()>,
{
    ForEach {
        stream: s,
        f: f,
        fut: None,
    }
}

impl<S, U, F> ForEach<S, U, F> {
    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(f -> F);
    unsafe_pinned!(fut -> Option<U>);
}

impl<S, U, F> Future for ForEach<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut task::Context) -> Poll<()> {
        loop {
            if let Some(fut) = self.fut().as_pin_mut() {
                try_ready!(fut.poll(cx));
            }
            self.fut().assign(None);

            match try_ready!(self.stream().poll_next(cx)) {
                Some(e) => {
                    let fut = (self.f())(e);
                    self.fut().assign(Some(fut));
                }
                None => {
                    return Poll::Ready(());
                }
            }
        }
    }
}
