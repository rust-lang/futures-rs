use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Poll, Context};

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
        f,
        fut: None,
    }
}

impl<S, U, F> ForEach<S, U, F> {
    unsafe_pinned!(stream -> S);
    unsafe_unpinned!(f -> F);
    unsafe_pinned!(fut -> Option<U>);
}

impl<S, U, F> Unpin for ForEach<S, U, F> {}

impl<S, U, F> Future for ForEach<S, U, F>
    where S: Stream,
          F: FnMut(S::Item) -> U,
          U: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: PinMut<Self>, cx: &mut Context) -> Poll<()> {
        loop {
            if let Some(fut) = self.fut().as_pin_mut() {
                ready!(fut.poll(cx));
            }
            PinMut::set(self.fut(), None);

            match ready!(self.stream().poll_next(cx)) {
                Some(e) => {
                    let fut = (self.f())(e);
                    PinMut::set(self.fut(), Some(fut));
                }
                None => {
                    return Poll::Ready(());
                }
            }
        }
    }
}
